use nix::fcntl::{OFlag, open};
use nix::sys::stat::Mode;
use nix::unistd::Pid;
use nix::{sys, unistd};
use std::env::temp_dir;
use std::fs::{self, File};
use std::io::Read as _;
use std::os::fd::{AsRawFd, FromRawFd};
use std::path::PathBuf;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::error::PipeError;
use crate::event::TraceEvent;

pub struct Pipe {
    path: PathBuf,
}

impl Pipe {
    pub fn new(target_symbol: &str, pid: Pid, suffix: Option<&str>) -> Result<Self, PipeError> {
        let tmpdir = temp_dir();
        // シンボル名のスラッシュをアンダースコアに置換してファイル名として安全にする
        // これをしないとディレクトリを事前に作る必要ができて面倒
        let target_symbol = target_symbol.replace('/', "_");
        let suffix = suffix.unwrap_or("").replace("/", "_");
        let pipe_dir = tmpdir.join("tracer");
        std::fs::create_dir_all(&pipe_dir).map_err(PipeError::FailedToCreateDirectory)?;
        let path = pipe_dir.join(format!("{}_{}_{}.pipe", pid, target_symbol, suffix));

        // 既存のパイプファイルが存在する場合は削除
        if path.exists() {
            let _ = fs::remove_file(&path);
        }

        unistd::mkfifo(&path, sys::stat::Mode::S_IRWXU).map_err(PipeError::FailedToMkfifo)?;
        println!("Created pipe on {path:?}");
        Ok(Self { path })
    }

    pub fn path(&self) -> &str {
        self.path.to_str().unwrap_or("")
    }

    pub fn start_reader(&self) -> JoinHandle<u64> {
        let pipe_path = self.path().to_string();

        thread::spawn(move || {
            println!(
                "Pipe reader thread started, waiting for data from: {}",
                pipe_path
            );

            // 非ブロッキングモードでパイプを開く
            let fd = loop {
                match open(pipe_path.as_str(), OFlag::O_RDONLY, Mode::empty()) {
                    Ok(fd) => break fd,
                    Err(e) => {
                        eprintln!("Failed to open pipe (retrying): {:?}", e);
                        thread::sleep(Duration::from_millis(100));
                    }
                }
            };

            println!("Pipe opened successfully in non-blocking mode");

            let mut file = unsafe { File::from_raw_fd(fd.as_raw_fd()) };
            let mut counter = 0u64;
            let mut buffer = vec![0u8; 0];
            let mut temp_buf = [0u8; 24];

            loop {
                match file.read(&mut temp_buf) {
                    Ok(n) if n > 0 => {
                        println!("Read {} bytes from pipe", n);
                        buffer.extend_from_slice(&temp_buf[..n]);
                        while buffer.len() >= 24 {
                            let event_bytes: [u8; 24] =
                                buffer.drain(..24).collect::<Vec<u8>>().try_into().unwrap();

                            match TraceEvent::from_bytes(&event_bytes) {
                                Ok(event) => {
                                    counter += 1;
                                    // Copy fields to locals to avoid unaligned references
                                    let x28 = event.x28_value;
                                    let ts = event.timestamp;
                                    println!(
                                        "[TRACE #{}] {:?} - x28: 0x{:016x}, Timestamp: 0x{:016x} ({})",
                                        counter, event.event_type, x28, ts, ts
                                    );
                                }
                                Err(e) => {
                                    eprintln!("Failed to parse trace event: {:?}", e);
                                }
                            }
                        }
                    }
                    Ok(_) => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(e) => {
                        println!("Pipe read error: {:?}", e);
                        break;
                    }
                }
            }
            println!("Pipe reader thread exiting after {} entries", counter);
            counter
        })
    }
}

impl Drop for Pipe {
    fn drop(&mut self) {
        println!("Pipe is dropped!");
        fs::remove_file(&self.path).expect("Failed to delete pipe");
    }
}

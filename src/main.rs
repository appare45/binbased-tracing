use clap::Parser;
use clap::Subcommand;
use nix::fcntl::{OFlag, open};
use nix::sys::stat::Mode;
use nix::sys::wait;
use std::fs::File;
use std::io::Read;
use std::os::fd::{AsRawFd, FromRawFd};
use std::process::Command;
use std::process::Stdio;
use std::thread;
use std::time::Duration;

mod conf;
mod elf;
mod error;
mod instruction;
mod instrument;
mod maps;
mod pipe;
mod proc;
mod ptrace;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Attach { pid: i32 },
    Exec { path: String, args: Vec<String> },
}

const TARGET_SYMBOL: &str = "net/http.serverHandler.ServeHTTP";

#[cfg(not(target_arch = "aarch64"))]
compile_error!("This crate only supports aarch64 architecture");

#[cfg(target_arch = "aarch64")]
fn main() {
    let c = match Cli::parse().command {
        Commands::Attach { pid } => {
            println!(
                "Attaching another process is not stable since the waiting for pid is not available"
            );
            conf::new(nix::unistd::Pid::from_raw(pid), false)
        }
        Commands::Exec { path, args } => {
            let mut command = Command::new(path);
            for arg in args {
                command.arg(arg);
            }
            let pid = command
                .stderr(Stdio::inherit())
                .stdout(Stdio::inherit())
                .spawn()
                .expect("Failed to spawn child process")
                .id();
            conf::new(nix::unistd::Pid::from_raw(pid as i32), true)
        }
    };
    let proc = c.trace().expect("Failed to initialize process trace");

    // ターゲットアドレスを取得
    let elf = proc.get_bin().expect("Failed to get bin");
    let exec_base = proc.exe_base().expect("Failed to get exe base");
    let off = elf
        .get_symbol(TARGET_SYMBOL.into())
        .expect("Failed to get symbol")
        .0;
    let target_addr = off + exec_base;
    println!("{TARGET_SYMBOL} is at 0x{target_addr:x}");

    let pipe = pipe::Pipe::new(TARGET_SYMBOL, proc.pid).expect("Failed to create pipe");

    let pipe_path = pipe.path().to_string();
    let reader_thread = thread::spawn(move || {
        println!(
            "Pipe reader thread started, waiting for data from: {}",
            pipe_path
        );
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
        let mut temp_buf = [0u8; 8];

        loop {
            match file.read(&mut temp_buf) {
                Ok(n) if n > 0 => {
                    println!("Read {} bytes from pipe", n);
                    buffer.extend_from_slice(&temp_buf[..n]);
                    while buffer.len() >= 8 {
                        let timestamp_bytes: [u8; 8] =
                            buffer.drain(..8).collect::<Vec<u8>>().try_into().unwrap();
                        let timestamp = u64::from_le_bytes(timestamp_bytes);
                        counter += 1;
                        println!(
                            "[TRACE #{}] Timestamp: 0x{:016x} ({})",
                            counter, timestamp, timestamp
                        );
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
    });

    let instrument = instrument::new(proc, target_addr, &pipe).expect("Failed to start instrument");
    let proc = instrument.instrument().expect("Failed to instrument");

    println!("Instrumentation complete. Waiting for program events...");

    loop {
        match proc.wait_for_status() {
            Ok(status) => match status {
                wait::WaitStatus::Exited(_, code) => {
                    println!("Program exited with {code}");
                    break;
                }
                status => println!("{status:?}"),
            },
            Err(err) => {
                println!("{err:?}");
                break;
            }
        };
    }
    println!("Waiting for pipe reader thread to finish...");
    drop(pipe); // パイプをクローズしてリーダーを終了させる
    let _ = reader_thread.join();
}

use clap::Parser;
use clap::Subcommand;
use nix::sys::wait;
use std::io::{Read, Seek, SeekFrom};
use std::process::Command;
use std::process::Stdio;

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
    let mut is_child_processs = false;
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
            is_child_processs = true;
            conf::new(nix::unistd::Pid::from_raw(pid as i32), true)
        }
    };
    let proc = c.trace().expect("Failed to initialize process trace");

    // ターゲットアドレスを取得
    let elf = proc.get_bin().expect("Failed to get bin");
    let exec_base = proc.exe_base().expect("Failed to get exe base");
    let (offset, size) = elf
        .get_symbol(TARGET_SYMBOL.into())
        .expect("Failed to get symbol");
    let target_addr = offset + exec_base;
    println!("{TARGET_SYMBOL} is at 0x{target_addr:x}");

    let mut exe = proc.get_exe().expect("Failed to open exe file");
    let mut buf = vec![0u8; size as usize];
    exe.seek(SeekFrom::Start(offset))
        .expect("Failed to seek function start");
    exe.read_exact(&mut buf).expect("Failed to read function");

    // ret命令を検出
    let ret_addrs: Vec<u64> = buf
        .chunks_exact(4)
        .enumerate()
        .filter_map(|(i, chunk)| {
            let inst = u32::from_le_bytes(chunk.try_into().unwrap());
            if inst == 0xd65f03c0 {
                Some(exec_base + offset + (i as u64 * 4))
            } else {
                None
            }
        })
        .collect();

    println!("Found {} ret instructions", ret_addrs.len());
    for (idx, addr) in ret_addrs.iter().enumerate() {
        println!("  ret #{}: 0x{:x}", idx + 1, addr);
    }

    let pipe_entry =
        pipe::Pipe::new(TARGET_SYMBOL, proc.pid, Some("entry")).expect("Failed to create pipe");

    // パイプからデータを読み取るスレッドを起動
    let reader_thread = pipe_entry.start_reader();

    let pipe_end =
        pipe::Pipe::new(TARGET_SYMBOL, proc.pid, Some("end")).expect("Failed to create pipe");

    let mut targets = vec![instrument::InstrumentTarget {
        addr: target_addr,
        builder: Box::new(instruction::EntryTrampolineBuilder()),
        pipe_path: pipe_entry.path().to_string(),
    }];

    let reader_thread2 = pipe_end.start_reader();

    for ret_addr in ret_addrs {
        targets.push(instrument::InstrumentTarget {
            addr: ret_addr,
            builder: Box::new(instruction::EntryTrampolineBuilder()),
            pipe_path: pipe_end.path().to_string(),
        });
    }

    let instrument = instrument::new(proc, targets).expect("Failed to start instrument");
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
                if is_child_processs {
                    println!("{err:?}");
                    break;
                }
            }
        };
    }

    println!("Waiting for pipe reader thread to finish...");
    drop(pipe_entry); // パイプをクローズしてリーダーを終了させる
    drop(pipe_end);
    let _ = reader_thread.join();
    let _ = reader_thread2.join();
}

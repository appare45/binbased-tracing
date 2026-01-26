use clap::Parser;
use clap::Subcommand;
use nix::sys::wait;
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

    let _pipe = pipe::Pipe::new(TARGET_SYMBOL, proc.pid).expect("Failed to create pipe");

    // ここでprocを消費する
    let instrument = instrument::new(proc, target_addr).expect("Failed to start instrument");
    let proc = instrument.instrument().expect("Failed to instrument");
    loop {
        match proc.wait_for_status() {
            Ok(status) => match status {
                wait::WaitStatus::Exited(_, code) => {
                    println!("Program exited with {code}");
                }
                status => println!("{status:?}"),
            },
            Err(err) => {
                println!("{err:?}");
                break;
            }
        };
    }
}

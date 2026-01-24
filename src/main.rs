use clap::Parser;
use clap::Subcommand;
use std::process::Command;
use std::process::Stdio;

mod conf;
mod elf;
mod error;
mod instrument;
mod maps;
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
        Commands::Attach { pid } => conf::new(nix::unistd::Pid::from_raw(pid), false),
        Commands::Exec { path, args } => {
            let mut command = Command::new(path);
            for arg in args {
                command.arg(arg);
            }
            let pid = command
                .stderr(Stdio::inherit())
                .stdout(Stdio::inherit())
                .spawn()
                .unwrap()
                .id();
            conf::new(nix::unistd::Pid::from_raw(pid as i32), true)
        }
    };
    let mut proc = c.trace().unwrap();
    let elf = proc.get_bin().unwrap();
    let exec_base = proc.exe_base().unwrap();
    let (off, _sym) = elf.get_symbol(TARGET_SYMBOL.into()).unwrap();
    let addr = off + exec_base;
    println!("{TARGET_SYMBOL} is at 0x{addr:x}");

    // ここでprocを消費する
    let ptrace = ptrace::Tracee::try_from(proc).unwrap();
    let instrument = instrument::Instrument::from(ptrace);
    let _proc = instrument.pre_instrument().unwrap().instrument().unwrap();
}

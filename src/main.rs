use clap::Parser;
use clap::Subcommand;
use std::process::Command;
use std::process::Stdio;

mod conf;
mod elf;
mod error;
mod instruction;
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
    let proc = c.trace().unwrap();

    // ここでprocを消費する
    let instrument = instrument::NotInstrumented::try_from(proc).unwrap();
    let _proc = instrument.instrument().unwrap();
}

use clap::Parser;
use clap::Subcommand;
use std::process::Command;
use std::process::Stdio;

mod conf;
mod elf;
mod error;
mod event;
mod instruction;
mod instrument;
mod maps;
mod monitor;
mod pipe;
mod proc;
mod ptrace;
mod symbol_analyzer;

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
    let (c, proc, is_child) = setup_process().expect("Failed to setup process");
    let analysis =
        symbol_analyzer::analyze_function(&proc, TARGET_SYMBOL).expect("Failed to analyze symbol");
    let plan = instrument::plan_instrumentation(&proc, &analysis, TARGET_SYMBOL)
        .expect("Failed to plan instrumentation");

    let inst = instrument::new(proc, plan.targets).expect("Failed to create instrument");
    let proc = inst.instrument().expect("Failed to instrument");

    monitor::monitor_process(&proc, is_child, plan.pipes, plan.readers)
        .expect("Failed to monitor process");

    // これをするとdropするタイミングをコンパイルするときにここまで生きていることを知れる
    drop(c);
}

fn setup_process() -> Result<(conf::Conf, proc::Proc, bool), Box<dyn std::error::Error>> {
    let (c, is_child) = match Cli::parse().command {
        Commands::Attach { pid } => {
            println!(
                "Attaching another process is not stable since the waiting for pid is not available"
            );
            (conf::new(nix::unistd::Pid::from_raw(pid), false), false)
        }
        Commands::Exec { path, args } => {
            let mut command = Command::new(path);
            for arg in args {
                command.arg(arg);
            }
            let pid = command
                .stderr(Stdio::inherit())
                .stdout(Stdio::inherit())
                .spawn()?
                .id();
            (
                conf::new(nix::unistd::Pid::from_raw(pid as i32), true),
                true,
            )
        }
    };
    let proc = c.trace()?;
    Ok((c, proc, is_child))
}

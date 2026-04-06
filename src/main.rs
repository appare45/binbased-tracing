use clap::Parser;
use clap::Subcommand;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;

mod conf;
mod config;
mod dwarf;
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
mod trace_collector;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Attach {
        pid: i32,
        #[arg(short, long, default_value = "trace.toml")]
        config: PathBuf,
    },
    Exec {
        path: String,
        args: Vec<String>,
        #[arg(short, long, default_value = "trace.toml")]
        config: PathBuf,
    },
}

#[cfg(not(target_arch = "aarch64"))]
compile_error!("This crate only supports aarch64 architecture");

#[cfg(target_arch = "aarch64")]
fn main() {
    use std::sync::mpsc::channel;

    let (c, proc, is_child, config) = setup_process().expect("Failed to setup process");

    // Create channel for trace events
    let (event_tx, event_rx) = channel();

    let mut all_pipes = Vec::new();
    let mut all_readers = Vec::new();
    let mut runtime_offsets = None;

    for target_symbol in &config.targets {
        let analysis = symbol_analyzer::analyze_function(&proc, target_symbol)
            .expect("Failed to analyze symbol");

        let plan = instrument::plan_instrumentation(&proc, &analysis, target_symbol, event_tx.clone())
            .expect("Failed to plan instrumentation");

        all_pipes.extend(plan.pipes);
        all_readers.extend(plan.readers);

        if runtime_offsets.is_none() {
            runtime_offsets = Some(plan.runtime_offsets);
        }

        let inst = instrument::new(proc, plan.targets, runtime_offsets.clone().unwrap())
            .expect("Failed to create instrument");
        let _ = inst.instrument().expect("Failed to instrument");
    }

    monitor::monitor_process(&proc, is_child, all_pipes, all_readers, event_rx)
        .expect("Failed to monitor process");

    // これをするとdropするタイミングをコンパイルするときにここまで生きていることを知れる
    drop(c);
}

fn setup_process() -> Result<(conf::Conf, proc::Proc, bool, config::Config), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let (c, is_child, config_path) = match cli.command {
        Commands::Attach { pid, config } => {
            println!(
                "Attaching another process is not stable since the waiting for pid is not available"
            );
            (conf::new(nix::unistd::Pid::from_raw(pid), false), false, config)
        }
        Commands::Exec { path, args, config } => {
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
                config,
            )
        }
    };
    let config = config::load(&config_path)?;
    let proc = c.trace()?;
    Ok((c, proc, is_child, config))
}

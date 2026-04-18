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
mod proc;
mod event_buffer;
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
    use std::collections::HashMap;
    use std::sync::mpsc::channel;
    use event::{SymbolId, SymbolInfo};

    let (c, proc, is_child, config, mut buffer) = setup_process().expect("Failed to setup process");

    let (event_tx, event_rx) = channel();

    let mut all_targets = Vec::new();
    let mut runtime_offsets = None;
    let mut symbol_map: HashMap<SymbolId, SymbolInfo> = HashMap::new();

    for (idx, target_symbol) in config.targets.iter().enumerate() {
        let symbol_id = SymbolId(idx as u16);
        let analysis = symbol_analyzer::analyze_function(&proc, target_symbol)
            .expect("Failed to analyze symbol");

        let plan = instrument::plan_instrumentation(
            &proc,
            &analysis,
            &mut buffer,
            symbol_id,
            event_tx.clone(),
        )
        .expect("Failed to plan instrumentation");

        all_targets.extend(plan.targets);

        if runtime_offsets.is_none() {
            runtime_offsets = Some(plan.runtime_offsets);
        }

        symbol_map.insert(symbol_id, SymbolInfo { name: target_symbol.clone() });
    }

    let inst = instrument::new(proc, all_targets, runtime_offsets.unwrap())
        .expect("Failed to create instrument");
    let proc = inst.instrument().expect("Failed to instrument");

    monitor::monitor_process(&proc, is_child, buffer, event_rx, symbol_map)
        .expect("Failed to monitor process");

    drop(c);
}

fn setup_process() -> Result<
    (conf::Conf, proc::Proc, bool, config::Config, event_buffer::EventBuffer),
    Box<dyn std::error::Error>,
> {
    use event_buffer::EventBuffer;

    let cli = Cli::parse();

    match cli.command {
        Commands::Attach { pid, config } => {
            println!(
                "Attaching another process is not stable since the waiting for pid is not available"
            );
            println!("Attached to process with PID: {}", pid);
            let config = config::load(&config)?;
            let c = conf::new(nix::unistd::Pid::from_raw(pid), false);
            let proc = c.trace()?;
            let buffer = EventBuffer::create()?;
            Ok((c, proc, false, config, buffer))
        }
        Commands::Exec { path, args, config } => {
            let config = config::load(&config)?;
            let buffer = EventBuffer::create()?;

            let mut command = Command::new(path);
            for arg in args {
                command.arg(arg);
            }

            let pid = command
                .stderr(Stdio::inherit())
                .stdout(Stdio::inherit())
                .spawn()?
                .id();
            println!("Spawned process with PID: {}", pid);

            let c = conf::new(nix::unistd::Pid::from_raw(pid as i32), true);
            let proc = c.trace()?;
            Ok((c, proc, true, config, buffer))
        }
    }
}

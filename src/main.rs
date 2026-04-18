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
    use std::sync::mpsc::channel;

    let (c, proc, is_child, config, pre_buffers) = setup_process().expect("Failed to setup process");

    let (event_tx, event_rx) = channel();

    let mut all_targets = Vec::new();
    let mut all_buffers = Vec::new();
    let mut runtime_offsets = None;

    for (target_symbol, buffers) in config.targets.iter().zip(pre_buffers.into_iter()) {
        let analysis = symbol_analyzer::analyze_function(&proc, target_symbol)
            .expect("Failed to analyze symbol");

        let plan = instrument::plan_instrumentation(&proc, &analysis, target_symbol, buffers, event_tx.clone())
            .expect("Failed to plan instrumentation");

        all_targets.extend(plan.targets);
        all_buffers.extend(plan.buffers);

        if runtime_offsets.is_none() {
            runtime_offsets = Some(plan.runtime_offsets);
        }
    }

    let inst = instrument::new(proc, all_targets, runtime_offsets.unwrap())
        .expect("Failed to create instrument");
    let proc = inst.instrument().expect("Failed to instrument");

    monitor::monitor_process(&proc, is_child, all_buffers, event_rx)
        .expect("Failed to monitor process");

    drop(c);
}

fn setup_process() -> Result<
    (conf::Conf, proc::Proc, bool, config::Config, Vec<Vec<event_buffer::EventBuffer>>),
    Box<dyn std::error::Error>,
> {
    use event_buffer::EventBuffer;
    use std::path::PathBuf;

    let cli = Cli::parse();

    match cli.command {
        Commands::Attach { pid, config } => {
            println!(
                "Attaching another process is not stable since the waiting for pid is not available"
            );
            println!("Attached to process with PID: {}", pid);
            let config = config::load(&config)?;
            // attach の場合は spawn 前バッファ作成不可なので spawn 後に作成（将来対応）
            let c = conf::new(nix::unistd::Pid::from_raw(pid), false);
            let proc = c.trace()?;
            let mut pre_buffers: Vec<Vec<EventBuffer>> = Vec::new();
            for _ in &config.targets {
                pre_buffers.push(vec![EventBuffer::create()?]);
            }
            Ok((c, proc, false, config, pre_buffers))
        }
        Commands::Exec { path, args, config } => {
            let exe_path = PathBuf::from(&path);
            let config = config::load(&config)?;

            // spawn前にバッファを作成（memfdをfork前に確保してfdを子に引き継ぐ）
            let mut pre_buffers: Vec<Vec<EventBuffer>> = Vec::new();
            for target_symbol in &config.targets {
                let n = symbol_analyzer::count_buffers_needed(&exe_path, target_symbol)
                    .unwrap_or(1);
                let mut bufs = Vec::new();
                for _ in 0..n {
                    bufs.push(EventBuffer::create()?);
                }
                pre_buffers.push(bufs);
            }

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
            Ok((c, proc, true, config, pre_buffers))
        }
    }
}

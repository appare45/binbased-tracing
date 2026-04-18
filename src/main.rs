use std::sync::Arc;
use binbased_tracing::{config, event, event_buffer, instrument, monitor, proc};
use clap::Parser;
use clap::Subcommand;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[arg(short, long, default_value = "trace.toml")]
    config: PathBuf,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Attach {
        pid: i32,
    },
    Exec {
        path: String,
        args: Vec<String>,
    },
}

#[cfg(not(target_arch = "aarch64"))]
compile_error!("This crate only supports aarch64 architecture");

#[cfg(target_arch = "aarch64")]
fn main() {
    let cli = Cli::parse();
    let config = config::load(&cli.config).expect("Failed to load config");
    let (proc, buffer, event_rx) = setup_process(cli).expect("Failed to setup process");
    let buffer = Arc::new(buffer);

    let mut inst = instrument::Instrumenter::new(proc).expect("Failed to create instrumenter");
    let mut registry = event::TargetRegistry::new();
    for target in &config.targets {
        let analysis = target.analyze(&inst.proc).expect("Failed to analyze function");
        let id = registry.add(target.name.clone());
        inst = inst.add_target(&analysis, Arc::clone(&buffer), id).expect("Failed to add target");
    }

    monitor::monitor_process(
        inst.proc,
        buffer,
        event_rx,
        registry,
    )
    .expect("Failed to monitor process");
}

fn setup_process(cli: Cli) -> Result<
    (proc::Proc, event_buffer::EventBuffer, event_buffer::EventReceiver),
    Box<dyn std::error::Error>,
> {
    let (buffer, event_rx) = event_buffer::EventBuffer::create()?;

    match cli.command {
        Commands::Attach { pid } => {
            println!(
                "Attaching another process is not stable since the waiting for pid is not available"
            );
            println!("Attached to process with PID: {}", pid);
            let proc = proc::new(nix::unistd::Pid::from_raw(pid), false)?;
            Ok((proc, buffer, event_rx))
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
            println!("Spawned process with PID: {}", pid);

            let proc = proc::new(nix::unistd::Pid::from_raw(pid as i32), true)?;
            Ok((proc, buffer, event_rx))
        }
    }
}

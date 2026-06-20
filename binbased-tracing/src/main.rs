use std::fs;
use std::sync::{Arc, RwLock};
use std::time::SystemTime;
use binbased_tracing::{config, event, event_buffer, instrument, monitor, proc};
use clap::Parser;
use clap::Subcommand;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[arg(short, long, default_value = "trace.yaml")]
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
    let config_path = cli.config.clone();
    let (proc, mut buffer) = setup_process(cli).expect("Failed to setup process");
    let event_rx = buffer.take_receiver();
    let buffer = Arc::new(buffer);

    let mut inst = Some(instrument::Instrumenter::new(proc).expect("Failed to create instrumenter"));
    let registry = Arc::new(RwLock::new(event::TargetRegistry::new()));

    let done_rx = monitor::start(event_rx, Arc::clone(&registry));

    let mut last_mtime: Option<SystemTime> = None;
    loop {
        let mtime = fs::metadata(&config_path)
            .and_then(|m| m.modified())
            .ok();

        if mtime != last_mtime {
            last_mtime = mtime;
            match config::load(&config_path) {
                Ok(cfg) => {
                    for target in &cfg.targets {
                        if registry.read().unwrap().contains(&target.name) {
                            continue;
                        }
                        let Some(current_inst) = inst.take() else { break };
                        match target.analyze(&current_inst.proc) {
                            Ok(analysis) => {
                                let id = registry.write().unwrap().add(target.name.clone());
                                match current_inst.add_target(&analysis, Arc::clone(&buffer), id) {
                                    Ok(new_inst) => {
                                        inst = Some(new_inst);
                                        println!("Instrumented: {}", target.name);
                                    }
                                    Err(e) => {
                                        eprintln!("Failed to instrument {}: {e}", target.name);
                                        break;
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("Failed to analyze {}: {e}", target.name);
                                inst = Some(current_inst);
                            }
                        }
                    }
                }
                Err(e) => eprintln!("Failed to load config: {e}"),
            }
        }

        if let Some(status) = inst.as_ref().and_then(|i| i.proc.try_wait()) {
            println!("Program exited: {status:?}");
            break;
        }

        std::thread::sleep(std::time::Duration::from_secs(1));
    }

    drop(buffer);
    let _ = done_rx.recv();
}

fn setup_process(cli: Cli) -> Result<
    (proc::Proc, event_buffer::EventBuffer),
    Box<dyn std::error::Error>,
> {
    let buffer = event_buffer::EventBuffer::create()?;

    match cli.command {
        Commands::Attach { pid } => {
            println!(
                "Attaching another process is not stable since the waiting for pid is not available"
            );
            println!("Attached to process with PID: {}", pid);
            let proc = proc::new(nix::unistd::Pid::from_raw(pid), false)?;
            Ok((proc, buffer))
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
            Ok((proc, buffer))
        }
    }
}

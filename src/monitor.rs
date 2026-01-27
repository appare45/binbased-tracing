use crate::pipe::Pipe;
use crate::proc::Proc;
use nix::sys::wait;
use std::thread::JoinHandle;

pub fn monitor_process(
    proc: &Proc,
    is_child: bool,
    pipes: Vec<Pipe>,
    readers: Vec<JoinHandle<u64>>,
) -> Result<(), Box<dyn std::error::Error>> {
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
                if is_child {
                    println!("{err:?}");
                    break;
                }
            }
        };
    }

    println!("Waiting for pipe reader thread to finish...");
    drop(pipes);
    for reader in readers {
        let _ = reader.join();
    }

    Ok(())
}

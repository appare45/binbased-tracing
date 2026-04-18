use std::sync::Arc;
use crate::error::MonitorError;
use crate::event::TargetRegistry;
use crate::event_buffer::{EventBuffer, EventReceiver};
use crate::proc::Proc;
use crate::trace_collector::TraceCollector;
use nix::sys::wait;
use std::sync::mpsc;
use std::thread;

pub fn monitor_process(
    proc: Proc,
    buffer: Arc<EventBuffer>,
    event_rx: EventReceiver,
    registry: TargetRegistry,
) -> Result<(), MonitorError> {
    println!("Instrumentation complete. Waiting for program events...");

    let collector_handle = thread::spawn(move || {
        let mut collector = TraceCollector::new(registry);
        while let Ok(event) = event_rx.recv() {
            collector.process_event(event);
        }
        collector
    });

    let pid = proc.pid;
    let (proc_done_tx, proc_done_rx) = mpsc::channel();
    thread::spawn(move || {
        loop {
            match wait::waitpid(pid, None) {
                Ok(status) => match status {
                    wait::WaitStatus::Exited(_, code) => {
                        println!("Program exited with {code}");
                        break;
                    }
                    wait::WaitStatus::Signaled(_, signal, _) => {
                        println!("Program signaled: {:?}", signal);
                        break;
                    }
                    status => println!("{status:?}"),
                },
                Err(err) => {
                    println!("{err:?}");
                    break;
                }
            };
        }
        let _ = proc_done_tx.send(());
    });

    let _ = proc_done_rx.recv();

    drop(buffer);

    // コレクタースレッドの終了を待つ
    collector_handle
        .join()
        .map_err(|_| MonitorError::CollectorThreadJoinFailed)?;

    Ok(())
}

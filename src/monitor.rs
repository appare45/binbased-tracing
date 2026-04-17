use crate::error::MonitorError;
use crate::event::TraceEvent;
use crate::proc::Proc;
use crate::event_buffer::EventBuffer;
use crate::trace_collector::TraceCollector;
use nix::sys::wait;
use std::sync::mpsc::{self, Receiver};
use std::thread;

pub fn monitor_process(
    proc: &Proc,
    is_child: bool,
    buffers: Vec<EventBuffer>,
    event_rx: Receiver<TraceEvent>,
) -> Result<(), MonitorError> {
    println!("Instrumentation complete. Waiting for program events...");

    let collector_handle = thread::spawn(move || {
        let mut collector = TraceCollector::new();
        while let Ok(event) = event_rx.recv() {
            collector.process_event(event);
        }
        collector
    });

    // プロセス状態監視スレッド
    let proc = *proc; // Copyなのでデリファレンスでコピー
    let (proc_done_tx, proc_done_rx) = mpsc::channel();
    thread::spawn(move || {
        loop {
            match proc.wait_for_status() {
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
                    if is_child {
                        println!("{err:?}");
                        break;
                    }
                }
            };
        }
        let _ = proc_done_tx.send(());
    });

    let _ = proc_done_rx.recv();

    drop(buffers);

    // コレクタースレッドの終了を待つ
    collector_handle
        .join()
        .map_err(|_| MonitorError::CollectorThreadJoinFailed)?;

    Ok(())
}

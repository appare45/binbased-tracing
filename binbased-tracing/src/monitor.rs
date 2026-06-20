use crate::event::TargetRegistry;
use crate::event_buffer::EventReceiver;
use crate::trace_collector::TraceCollector;
use std::sync::mpsc;
use std::sync::{Arc, RwLock};
use std::thread;

/// イベント収集スレッドを起動し、完了通知用のchannelを返す
pub fn start(event_rx: EventReceiver, registry: Arc<RwLock<TargetRegistry>>) -> mpsc::Receiver<()> {
    println!("Instrumentation complete. Waiting for program events...");

    let collector_handle = thread::spawn(move || {
        let mut collector = TraceCollector::new(registry);
        while let Ok(event) = event_rx.recv() {
            collector.process_event(event);
        }
    });

    let (done_tx, done_rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = collector_handle.join();
        let _ = done_tx.send(());
    });

    done_rx
}

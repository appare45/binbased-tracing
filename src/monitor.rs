use crate::error::MonitorError;
use crate::event::TraceEvent;
use crate::pipe::Pipe;
use crate::proc::Proc;
use crate::trace_collector::TraceCollector;
use nix::sys::wait;
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};
use std::time::Duration;

pub fn monitor_process(
    proc: &Proc,
    is_child: bool,
    pipes: Vec<Pipe>,
    readers: Vec<JoinHandle<u64>>,
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

    // パイプリーダーの終了を監視するチャネル
    let (reader_done_tx, reader_done_rx) = mpsc::channel();

    // パイプリーダーの終了を監視するスレッド
    thread::spawn(move || {
        for reader in readers {
            let _ = reader.join();
        }
        println!("All pipe readers finished");
        let _ = reader_done_tx.send(());
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

    // パイプリーダーまたはプロセスのどちらかが終了したら継続
    loop {
        if reader_done_rx.try_recv().is_ok() {
            println!("Pipes closed, stopping program...");
            break;
        }
        if proc_done_rx.try_recv().is_ok() {
            println!("Program ended");
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }

    // パイプをクリーンアップ
    drop(pipes);

    // コレクタースレッドの終了を待つ
    collector_handle
        .join()
        .map_err(|_| MonitorError::CollectorThreadJoinFailed)?;

    Ok(())
}

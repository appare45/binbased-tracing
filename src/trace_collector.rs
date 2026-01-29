use crate::event::{EventType, TraceEvent};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct EntryData {
    pub timestamp: u64,
}

pub struct TraceCollector {
    pending_entries: HashMap<u64, EntryData>,
    orphaned_returns: u64,
}

impl TraceCollector {
    pub fn new() -> Self {
        Self {
            pending_entries: HashMap::new(),
            orphaned_returns: 0,
        }
    }

    pub fn process_event(&mut self, event: TraceEvent) {
        let goroutine_id = event.goroutine;
        let timestamp = event.timestamp;

        match event.event_type {
            EventType::Entry => {
                let entry = EntryData { timestamp };

                if let Some(old) = self.pending_entries.insert(goroutine_id, entry.clone()) {
                    eprintln!(
                        "Warning: Goroutine {} had pending entry at {}, overwriting with new entry at {}",
                        goroutine_id, old.timestamp, timestamp
                    );
                }

                println!(
                    "[Entry] Goroutine 0x{:x} entered at timestamp {}",
                    goroutine_id, timestamp
                );
            }
            EventType::Return => {
                if let Some(entry) = self.pending_entries.remove(&goroutine_id) {
                    let duration = timestamp.saturating_sub(entry.timestamp);

                    println!(
                        "[Completed] Goroutine 0x{:x}: entry={}, return={}, duration={} cycles",
                        goroutine_id, entry.timestamp, timestamp, duration
                    );
                } else {
                    self.orphaned_returns += 1;
                    eprintln!(
                        "Warning: Return event for goroutine 0x{:x} without matching entry (orphaned returns: {})",
                        goroutine_id, self.orphaned_returns
                    );
                }
            }
        }
    }
}

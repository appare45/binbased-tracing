use crate::event::{EventType, TargetId, TargetRegistry, TraceEvent};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct EntryData {
    pub timestamp: u64,
}

pub struct TraceCollector {
    pending_entries: HashMap<(u64, TargetId), EntryData>,
    registry: TargetRegistry,
    orphaned_returns: u64,
}

impl TraceCollector {
    pub fn new(registry: TargetRegistry) -> Self {
        Self {
            pending_entries: HashMap::new(),
            registry,
            orphaned_returns: 0,
        }
    }

    pub fn process_event(&mut self, event: TraceEvent) {
        let goroutine_id = event.goroutine;
        let timestamp = event.timestamp;
        let target_id = event.target_id;
        let name = self.registry.name(target_id).to_owned();

        match event.event_type {
            EventType::Entry => {
                let entry = EntryData { timestamp };

                if let Some(old) = self.pending_entries.insert((goroutine_id, target_id), entry.clone()) {
                    eprintln!(
                        "Warning: Goroutine {} target '{}' had pending entry at {}, overwriting with new entry at {}",
                        goroutine_id, name, old.timestamp, timestamp
                    );
                }

                println!(
                    "[Entry] target='{}' goroutine=0x{:x} timestamp={}",
                    name, goroutine_id, timestamp
                );
            }
            EventType::Return => {
                if let Some(entry) = self.pending_entries.remove(&(goroutine_id, target_id)) {
                    let duration = timestamp.saturating_sub(entry.timestamp);

                    println!(
                        "[Completed] target='{}' goroutine=0x{:x}: entry={}, return={}, duration={} cycles",
                        name, goroutine_id, entry.timestamp, timestamp, duration
                    );
                } else {
                    self.orphaned_returns += 1;
                    eprintln!(
                        "Warning: Return event for goroutine 0x{:x} target '{}' without matching entry (orphaned returns: {})",
                        goroutine_id, name, self.orphaned_returns
                    );
                }
            }
        }
    }
}

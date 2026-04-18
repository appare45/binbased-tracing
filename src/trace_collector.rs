use crate::event::{EventType, SymbolId, SymbolInfo, TraceEvent};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct EntryData {
    pub timestamp: u64,
}

pub struct TraceCollector {
    pending_entries: HashMap<(u64, SymbolId), EntryData>,
    symbol_map: HashMap<SymbolId, SymbolInfo>,
    orphaned_returns: u64,
}

impl TraceCollector {
    pub fn new(symbol_map: HashMap<SymbolId, SymbolInfo>) -> Self {
        Self {
            pending_entries: HashMap::new(),
            symbol_map,
            orphaned_returns: 0,
        }
    }

    fn symbol_name(&self, id: SymbolId) -> &str {
        self.symbol_map
            .get(&id)
            .map(|s| s.name.as_str())
            .unwrap_or("<unknown>")
    }

    pub fn process_event(&mut self, event: TraceEvent) {
        let goroutine_id = event.goroutine;
        let timestamp = event.timestamp;
        let symbol_id = event.symbol_id;
        let name = self.symbol_name(symbol_id).to_owned();

        match event.event_type {
            EventType::Entry => {
                let entry = EntryData { timestamp };

                if let Some(old) = self.pending_entries.insert((goroutine_id, symbol_id), entry.clone()) {
                    eprintln!(
                        "Warning: Goroutine {} symbol '{}' had pending entry at {}, overwriting with new entry at {}",
                        goroutine_id, name, old.timestamp, timestamp
                    );
                }

                println!(
                    "[Entry] symbol='{}' goroutine=0x{:x} timestamp={}",
                    name, goroutine_id, timestamp
                );
            }
            EventType::Return => {
                if let Some(entry) = self.pending_entries.remove(&(goroutine_id, symbol_id)) {
                    let duration = timestamp.saturating_sub(entry.timestamp);

                    println!(
                        "[Completed] symbol='{}' goroutine=0x{:x}: entry={}, return={}, duration={} cycles",
                        name, goroutine_id, entry.timestamp, timestamp, duration
                    );
                } else {
                    self.orphaned_returns += 1;
                    eprintln!(
                        "Warning: Return event for goroutine 0x{:x} symbol '{}' without matching entry (orphaned returns: {})",
                        goroutine_id, name, self.orphaned_returns
                    );
                }
            }
        }
    }
}

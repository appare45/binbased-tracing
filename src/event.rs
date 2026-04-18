#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct SymbolId(pub u16);

pub struct SymbolInfo {
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
#[allow(dead_code)]
pub enum EventType {
    Entry = 0,
    Return = 1,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct TraceEvent {
    pub event_type: EventType,
    pub _padding: [u8; 5],
    pub symbol_id: SymbolId,
    pub goroutine: u64,
    pub timestamp: u64,
}

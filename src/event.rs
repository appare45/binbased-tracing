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
    pub _padding: [u8; 7],
    pub goroutine: u64,
    pub timestamp: u64,
}

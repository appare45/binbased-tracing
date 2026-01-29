use crate::error::EventError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum EventType {
    Entry = 0,
    Return = 1,
}

impl EventType {
    pub fn from_u8(value: u8) -> Result<Self, EventError> {
        match value {
            0 => Ok(EventType::Entry),
            1 => Ok(EventType::Return),
            _ => Err(EventError::InvalidEventType(value)),
        }
    }
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct TraceEvent {
    pub event_type: EventType,
    pub _padding: [u8; 7],
    pub goroutine: u64,
    pub timestamp: u64,
}

impl TraceEvent {
    pub fn from_bytes(bytes: &[u8; 24]) -> Result<Self, EventError> {
        let event_type = EventType::from_u8(bytes[0])?;

        let goroutine = u64::from_le_bytes([
            bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
        ]);

        let timestamp = u64::from_le_bytes([
            bytes[16], bytes[17], bytes[18], bytes[19], bytes[20], bytes[21], bytes[22], bytes[23],
        ]);

        Ok(TraceEvent {
            event_type,
            _padding: [0; 7],
            goroutine,
            timestamp,
        })
    }
}

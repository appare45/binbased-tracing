use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct TargetId(pub u16);

pub struct Target {
    pub name: String,
}

pub struct TargetRegistry {
    inner: HashMap<TargetId, Target>,
    next_id: u16,
}

impl TargetRegistry {
    pub fn new() -> Self {
        Self { inner: HashMap::new(), next_id: 0 }
    }

    pub fn add(&mut self, name: String) -> TargetId {
        let id = TargetId(self.next_id);
        self.next_id += 1;
        self.inner.insert(id, Target { name });
        id
    }

    pub fn contains(&self, name: &str) -> bool {
        self.inner.values().any(|t| t.name == name)
    }

    pub fn name(&self, id: TargetId) -> &str {
        self.inner.get(&id).map(|t| t.name.as_str()).unwrap_or("<unknown>")
    }
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
    pub target_id: TargetId,
    pub goroutine: u64,
    pub timestamp: u64,
}

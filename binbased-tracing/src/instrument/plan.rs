use crate::{
    error::InstrumentError,
    event::TargetId,
    event_buffer::EventBuffer,
    instruction::BranchStrategy,
    proc,
    symbol_analyzer::FunctionAnalysis,
};
use std::sync::Arc;
pub const TRAMPOLINE_SIZE: u64 = 1024;

pub struct InstrumentTarget {
    pub addr: u64,
    pub event_type: u8,
    pub trampoline_hint: u64,
    pub buffer: Arc<EventBuffer>,
    pub target_id: TargetId,
}

pub struct AllocatedTarget {
    pub patch: BranchStrategy,
    pub trampoline_addr: u64,
    pub child_buffer_addr: u64,
    pub event_type: u8,
    pub target_id: TargetId,
}

pub struct InstrumentationPlan {
    pub targets: Vec<InstrumentTarget>,
    pub runtime_offsets: crate::dwarf::RuntimeOffsets,
}

pub fn plan_instrumentation(
    proc: &mut proc::Proc,
    analysis: &FunctionAnalysis,
    buffer: Arc<EventBuffer>,
    target_id: TargetId,
) -> Result<InstrumentationPlan, InstrumentError> {
    let exe_path = proc.exe_path()?;
    let elf_bytes = std::fs::read(&exe_path)?;
    let elf = crate::elf::new(&elf_bytes)?;

    let runtime_offsets = elf.runtime_offsets.ok_or(InstrumentError::DwarfError(
        crate::error::DwarfError::NoDwarfInfo,
    ))?;

    let targets = std::iter::once((analysis.entry_addr, 0u8))
        .chain(analysis.ret_addrs.iter().map(|&a| (a, 1u8)))
        .map(|(addr, event_type)| InstrumentTarget {
            trampoline_hint: proc.find_free_region(addr, TRAMPOLINE_SIZE),
            addr,
            event_type,
            buffer: Arc::clone(&buffer),
            target_id,
        })
        .collect();

    Ok(InstrumentationPlan {
        targets,
        runtime_offsets,
    })
}

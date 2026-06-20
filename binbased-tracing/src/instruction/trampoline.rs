use super::{blob, patch::BranchStrategy, Instructions};

pub fn build_trampoline(
    patch: &BranchStrategy,
    buffer_addr: u64,
    replaced_addr: u64,
    trampoline_addr: u64,
    inst1: u32,
    inst2: u32,
    inst3: u32,
    inst4: u32,
    event_type: u8,
    target_id: crate::event::TargetId,
    runtime_offsets: &crate::dwarf::RuntimeOffsets,
) -> Instructions {
    let header_val: u64 = (event_type as u64) | ((target_id.0 as u64) << 48);
    let mut instructions =
        blob::build_trampoline_from_blob(header_val, runtime_offsets.goid, buffer_addr);

    let epilogue_addr = trampoline_addr + instructions.len() as u64 * 4;
    instructions.join(patch.build_epilogue(
        replaced_addr,
        epilogue_addr,
        inst1,
        inst2,
        inst3,
        inst4,
    ));

    instructions
}

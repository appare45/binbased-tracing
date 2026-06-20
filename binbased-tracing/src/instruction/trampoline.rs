use super::{Instructions, blob};
use crate::instrument::plan::AllocatedTarget;

pub fn build_trampoline(
    target: &AllocatedTarget,
    replaced_addr: u64,
    replaced_insts: [u32; 4],
    runtime_offsets: &crate::dwarf::RuntimeOffsets,
) -> Instructions {
    let header_val: u64 = (target.event_type as u64) | ((target.target_id.0 as u64) << 48);
    let mut instructions = blob::build_trampoline_from_blob(
        header_val,
        runtime_offsets.goid,
        target.child_buffer_addr,
    );

    let epilogue_addr = target.trampoline_addr + instructions.len() as u64 * 4;
    instructions.join(
        target
            .patch
            .build_epilogue(replaced_addr, epilogue_addr, replaced_insts),
    );

    instructions
}

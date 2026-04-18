use super::{
    encode::{build_large_mov, build_ldp, build_stp, load_field_from_register},
    patch::BranchStrategy,
    Instructions,
};

fn push_registers_to_stack() -> Instructions {
    let mut instructions = Instructions::new();
    for i in 0..15 {
        instructions.push(build_stp(i * 2, i * 2 + 1, 31, -16));
    }
    instructions
}

fn pop_registers_from_stack() -> Instructions {
    let mut instructions = Instructions::new();
    for i in (0..15).rev() {
        instructions.push(build_ldp(2 * i, 2 * i + 1, 31, 16));
    }
    instructions
}

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
    let mut instructions = Instructions::new();
    instructions.join(push_registers_to_stack());
    instructions.push(0xd10083ff); // sub sp, sp, #32

    let header_val: u64 = (event_type as u64) | ((target_id.0 as u64) << 48);
    instructions.join(build_large_mov(0, header_val));
    instructions.push(0xf90003e0); // str x0, [sp]

    instructions.join(load_field_from_register(0, 28, runtime_offsets.goid));
    instructions.push(0xf90007e0); // str x0, [sp, #8]

    instructions.push(0xd53be040); // mrs x0, cntvct_el0
    instructions.push(0xf9000be0); // str x0, [sp, #16]

    instructions.join(build_large_mov(9, buffer_addr));
    instructions.push(0xc8dffd2a); // ldar x10, [x9]
    instructions.push(0x9240194b); // and x11, x10, #127
    instructions.push(0xd37ced6c); // lsl x12, x11, #4
    instructions.push(0x8b0b0d8c); // add x12, x12, x11, lsl #3
    instructions.push(0x8b09018c); // add x12, x12, x9
    instructions.push(0x9101018c); // add x12, x12, #64
    instructions.push(0xf94003ed); // ldr x13, [sp]
    instructions.push(0xf900018d); // str x13, [x12]
    instructions.push(0xf94007ed); // ldr x13, [sp, #8]
    instructions.push(0xf900058d); // str x13, [x12, #8]
    instructions.push(0xf9400bed); // ldr x13, [sp, #16]
    instructions.push(0xf900098d); // str x13, [x12, #16]
    instructions.push(0x9100054a); // add x10, x10, #1
    instructions.push(0xc89ffd2a); // stlr x10, [x9]

    instructions.push(0x910083ff); // add sp, sp, #32
    instructions.join(pop_registers_from_stack());

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

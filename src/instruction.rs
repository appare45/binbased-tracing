#[derive(Clone)]
pub struct Instructions(Vec<u32>);

impl Instructions {
    pub fn new() -> Self {
        Instructions(Vec::new())
    }

    pub fn push(&mut self, inst: u32) {
        self.0.push(inst);
    }

    pub fn join(&mut self, value: Instructions) {
        value.0.iter().for_each(|v| self.push(*v));
    }

    pub fn get(&self, index: usize) -> Option<u32> {
        self.0.get(index).copied()
    }
}

impl Into<Vec<i64>> for Instructions {
    fn into(self) -> Vec<i64> {
        self.0
            .chunks_exact(2)
            .map(|chunk| {
                let low = chunk[0] as i64;
                let high = chunk[1] as i64;

                // Combine in little-endian order: low 32 bits first, then high 32 bits
                (high << 32) | (low & 0xFFFFFFFF)
            })
            .collect()
    }
}

impl From<Vec<i64>> for Instructions {
    fn from(value: Vec<i64>) -> Self {
        let mut instruction = Instructions::new();
        value.iter().for_each(|v| {
            let high = *v as u32;
            let low = (v >> 32) as u32;
            instruction.push(high);
            instruction.push(low);
        });
        instruction
    }
}

impl From<i64> for Instructions {
    fn from(value: i64) -> Self {
        let mut instruction = Instructions::new();
        let high = value as u32;
        let low = (value >> 32) as u32;
        instruction.push(high);
        instruction.push(low);
        instruction
    }
}

fn push_registers_to_stack() -> Instructions {
    let mut instructions = Instructions::new();

    // Save frame pointer and link register
    instructions.push(build_stp(29, 30, 31, -32)); // stp x29, x30, [sp, #-32]!
    // Save x18, x19 for alignment
    instructions.push(build_stp(18, 19, 31, -16)); // stp x18, x19, [sp, #-16]!

    for i in 0..9 {
        instructions.push(build_stp(i * 2, i * 2 + 1, 31, -16));
    }
    instructions
}

fn build_stp(rt: u32, rt2: u32, rn: u32, imm: i32) -> u32 {
    let imm7 = ((imm / 8) as u32) & 0x7F;
    0b1010_1001_10 << 22 | imm7 << 15 | (rt2 & 0x1F) << 10 | (rn & 0x1F) << 5 | (rt & 0x1F)
}

#[test]
fn build_stp_test() {
    assert_eq!(build_stp(0, 1, 31, -16), 0xa9bf07e0);
    assert_eq!(build_stp(16, 17, 31, -16), 0xa9bf47f0);
}

pub fn jump_to_abs(target_addr: u64) -> Instructions {
    let mut instructions = Instructions::new();
    // ldr x16, #8 (load from PC+8, which points to the .quad below)
    instructions.push(0x58000050u32);
    // br x16 (branch to address in x16)
    instructions.push(0xd61f0200u32);
    // .quad target_addr (8-byte absolute address in little-endian)
    let low = target_addr as u32;
    let high = (target_addr >> 32) as u32;
    instructions.push(low);
    instructions.push(high);
    instructions
}

fn pop_registers_from_stack() -> Instructions {
    let mut instructions = Instructions::new();
    for i in (0..9).rev() {
        instructions.push(build_ldp(2 * i, 2 * i + 1, 31, 16));
    }

    // Restore x18, x19
    instructions.push(build_ldp(18, 19, 31, 16)); // ldp x18, x19, [sp], #16
    // Restore frame pointer and link register
    // TODO: 32 to 16
    instructions.push(build_ldp(29, 30, 31, 32)); // ldp x29, x30, [sp], #32

    instructions
}

fn build_ldp(rt: u32, rt2: u32, rn: u32, imm: i32) -> u32 {
    let imm7 = ((imm / 8) as u32) & 0x7F;
    0b1010_1000_11 << 22 | imm7 << 15 | (rt2 & 0x1F) << 10 | (rn & 0x1F) << 5 | (rt & 0x1F)
}

#[test]
fn build_ldp_test() {
    assert_eq!(build_ldp(16, 17, 31, 16), 0xa8c147f0)
}

pub fn build_trampoline(
    replaced_addr: u64,
    inst1: u32,
    inst2: u32,
    inst3: u32,
    inst4: u32,
) -> Instructions {
    let mut instructions = Instructions::new();
    instructions.join(push_registers_to_stack());

    // Prepare buffer with 'A' on stack
    instructions.push(0xd2800820); // mov x0, #0x41 ('A')
    instructions.push(0xf81f0fe0); // str x0, [sp, #-16]!        (push 'A' to stack with alignment)

    // Setup write(1, sp, 1) syscall
    instructions.push(0xd2800020); // mov x0, #1      (fd = stdout)
    instructions.push(0x910003e1); // mov x1, sp      (buf = stack pointer)
    instructions.push(0xd2800022); // mov x2, #1      (count = 1)
    instructions.push(0xd2800808); // mov x8, #64     (syscall number for write)
    instructions.push(0xd4000001); // svc #0          (make syscall)

    // Clean up the buffer from stack
    instructions.push(0x910043ff); // add sp, sp, #16 (pop the buffer)

    instructions.join(pop_registers_from_stack());

    // Execute all 4 instructions that were overwritten by jump_to_abs (16 bytes total)
    instructions.push(inst1);
    instructions.push(inst2);
    instructions.push(inst3);
    instructions.push(inst4);

    // Jump to the instruction AFTER the 16 bytes we overwrote
    instructions.join(jump_to_abs(replaced_addr + 16));

    return instructions;
}

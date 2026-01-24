use crate::error::InstructionError;

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

    pub fn set(&mut self, index: usize, inst: u32) -> Result<(), InstructionError> {
        if index >= self.0.len() {
            return Err(InstructionError::IndexOutOfBounds);
        }
        self.0[index] = inst;
        Ok(())
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
    for i in 0..15 {
        let rt = i * 2;
        let rt2 = i * 2 + 1;
        let rn = 31; // sp is index 31 (0b11111)
        let imm7 = (16 * i) / 8; // Scale the offset by 8

        let instr = 0b1010_1001_00 << 22
            | (imm7 & 0x7F) << 15
            | (rt2 & 0x1F) << 10
            | (rn & 0x1F) << 5
            | (rt & 0x1F);
        instructions.push(instr);
    }
    instructions.push(0xf9007bfe); // push link register
    instructions
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
    for i in 0..15 {
        let rt = i * 2;
        let rt2 = i * 2 + 1;
        let rn = 31; // sp is index 31 (0b11111)
        let imm7 = (16 * i) / 8; // Scale the offset by 8

        let instr = 0b1010_1001_01 << 22
            | (imm7 & 0x7F) << 15
            | (rt2 & 0x1F) << 10
            | (rn & 0x1F) << 5
            | (rt & 0x1F);
        instructions.push(instr);
    }
    instructions.push(0xf9007bfe); // push link register
    instructions
}

pub fn build_trampoline(replaced_addr: u64, inst1: u32, inst2: u32, inst3: u32, inst4: u32) -> Instructions {
    let mut instructions = Instructions::new();
    instructions.join(push_registers_to_stack());
    // puts(A)
    instructions.push(0x52800820u32);
    instructions.push(0x381f0fe0u32);
    instructions.push(0xd2800020u32);
    instructions.push(0x910003e1u32);
    instructions.push(0xd2800022u32);
    instructions.push(0xd2800808u32);
    instructions.push(0xd4000001u32);
    instructions.push(0x910043ffu32);
    instructions.push(0xd65f03c0u32);
    instructions.join(pop_registers_from_stack());
    instructions.push(to_be_replaced);
    instructions.join(jump_to_abs(replaced_addr));
    // Jump to the instruction AFTER the 16 bytes we overwrote
    instructions.join(jump_to_abs(replaced_addr + 16));

    return instructions;
}

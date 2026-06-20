use super::Instructions;

pub enum BranchStrategy {
    Branch {
        patch_addr: u64,
        trampoline_addr: u64,
    },
    JumpToAbs {
        patch_addr: u64,
        trampoline_addr: u64,
    },
}

impl BranchStrategy {
    pub fn new(patch_addr: u64, trampoline_addr: u64) -> Self {
        let offset = (trampoline_addr as i64) - (patch_addr as i64);
        let offset_insns = offset / 4;
        if offset % 4 == 0 && (-(1 << 25)..(1 << 25)).contains(&offset_insns) {
            BranchStrategy::Branch {
                patch_addr,
                trampoline_addr,
            }
        } else {
            BranchStrategy::JumpToAbs {
                patch_addr,
                trampoline_addr,
            }
        }
    }

    pub fn patch_addr(&self) -> u64 {
        match self {
            BranchStrategy::Branch { patch_addr, .. } => *patch_addr,
            BranchStrategy::JumpToAbs { patch_addr, .. } => *patch_addr,
        }
    }

    pub fn apply(&self) -> (Instructions, u64) {
        let patch_addr = self.patch_addr();
        let instructions = match self {
            BranchStrategy::Branch {
                patch_addr,
                trampoline_addr,
            } => {
                let offset = (*trampoline_addr as i64) - (*patch_addr as i64);
                let offset_insns = offset / 4;
                let mut instructions = Instructions::new();
                let imm26 = (offset_insns as u32) & 0x3FF_FFFF;
                instructions.push(0x1400_0000 | imm26);
                instructions
            }
            BranchStrategy::JumpToAbs {
                trampoline_addr, ..
            } => {
                let mut instructions = Instructions::new();
                instructions.push(0x58000050u32); // ldr x16, #8
                instructions.push(0xd61f0200u32); // br x16
                instructions.push(*trampoline_addr as u32);
                instructions.push((*trampoline_addr >> 32) as u32);
                instructions
            }
        };
        (instructions, patch_addr)
    }

    pub(super) fn build_epilogue(
        &self,
        replaced_addr: u64,
        epilogue_addr: u64,
        insts: [u32; 4],
    ) -> Instructions {
        let mut instructions = Instructions::new();
        match self {
            BranchStrategy::Branch { patch_addr, .. } => {
                if insts[0] == 0xd65f03c0 {
                    instructions.push(0xd65f03c0); // ret
                } else {
                    instructions.join(relocate_instruction(insts[0]));
                    let b_addr = epilogue_addr + instructions.len() as u64 * 4;
                    instructions.join(BranchStrategy::new(b_addr, *patch_addr + 4).apply().0);
                }
            }
            BranchStrategy::JumpToAbs { .. } => {
                for &inst in insts.iter() {
                    instructions.join(relocate_instruction(inst));
                }
                instructions.join(
                    BranchStrategy::JumpToAbs {
                        patch_addr: 0,
                        trampoline_addr: replaced_addr + 16,
                    }
                    .apply()
                    .0,
                );
            }
        }
        instructions
    }
}

fn relocate_instruction(inst: u32) -> Instructions {
    let mut instructions = Instructions::new();
    instructions.push(inst);
    instructions
}

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

fn build_large_mov(target: u32, value: u64) -> Instructions {
    let mut instructions = Instructions::new();
    let mut first = true;

    for shift in (0..64).step_by(16) {
        let chunk = ((value >> shift) & 0xFFFF) as u32;
        if chunk == 0 && !first {
            continue;
        }

        if first {
            instructions.push(build_movz(target, chunk, shift as u32));
            first = false;
        } else {
            instructions.push(build_movk(target, chunk, shift as u32));
        }
    }

    instructions
}

fn build_movz(target: u32, value: u32, shift: u32) -> u32 {
    0b110100101 << 23 | ((shift / 16) & 0b11) << 21 | ((value & 0xFFFF) << 5) | (target & 0x1F)
}

fn build_movk(target: u32, value: u32, shift: u32) -> u32 {
    0b111100101 << 23 | ((shift / 16) & 0b11) << 21 | ((value & 0xFFFF) << 5) | (target & 0x1F)
}

#[test]
fn build_movz_test() {
    assert_eq!(build_movz(0, 0xffff, 0), 0xd29fffe0);
    assert_eq!(build_movz(0, 0x8b0f, 16), 0xd2b161e0);
    assert_eq!(build_movz(8, 0x40, 0), 0xd2800808)
}

#[test]
fn build_movk_test() {
    assert_eq!(build_movk(0, 0xffff, 0), 0xf29fffe0);
    assert_eq!(build_movk(0, 0x8b0f, 16), 0xf2b161e0);
}

pub fn build_trampoline(
    fifo_path_addr: u64,
    replaced_addr: u64,
    inst1: u32,
    inst2: u32,
    inst3: u32,
    inst4: u32,
) -> Instructions {
    let mut instructions = Instructions::new();
    instructions.join(push_registers_to_stack());

    // openat(-100, fifo_path_addr, O_WRONLY | O_NONBLOCK)
    instructions.join(build_large_mov(0, 0xFFFFFFFFFFFFFF9C)); // x0 = AT_FDCWD
    instructions.join(build_large_mov(1, fifo_path_addr)); // x1 = pathname
    instructions.push(build_movz(2, 0x801, 0)); // x2 = O_WRONLY | O_NONBLOCK (1 | 0x800)
    instructions.push(build_movz(8, 56, 0)); // x8 = openat
    instructions.push(0xd4000001); // svc #0

    // fdをスタックに保存
    instructions.push(0xf81f0fe0); // str x0, [sp, #-16]!

    // タイムスタンプを取得してスタックに保存
    instructions.push(0xd53be040); // mrs x0, cntvct_el0
    instructions.push(0xf81f0fe0); // str x0, [sp, #-16]!

    // write(fd, sp, 8) - タイムスタンプをパイプに書き込む
    // スタック配置: sp+0=timestamp, sp+16=fd (各strが16バイトプリデクリメント)
    instructions.push(0xf9400be0); // ldr x0, [sp, #16] (fd)
    instructions.push(0x910003e1); // mov x1, sp (タイムスタンプのアドレス)
    instructions.push(build_movz(2, 8, 0)); // x2 = 8
    instructions.push(build_movz(8, 64, 0)); // x8 = write
    instructions.push(0xd4000001); // svc #0

    // close(fd) - パイプを閉じる
    instructions.push(0xf9400be0); // ldr x0, [sp, #16] (fd)
    instructions.push(build_movz(8, 57, 0)); // x8 = close
    instructions.push(0xd4000001); // svc #0

    // スタックをクリーンアップ (timestamp + fd = 32バイト)
    instructions.push(0x910083ff); // add sp, sp, #32

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

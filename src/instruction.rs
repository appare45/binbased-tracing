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
    for i in (0..15).rev() {
        instructions.push(build_ldp(2 * i, 2 * i + 1, 31, 16));
    }

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

fn build_mov(rd: u32, rn: u32) -> u32 {
    // orr xd, xzr, xn
    // 形式: 1010101000nnnnnn000000111111ddddd
    0b10101010_00 << 22 | (rn & 0x1F) << 16 | 0b11111 << 5 | (rd & 0x1F)
}

fn build_ldr_offset(rd: u32, rn: u32, offset: u64) -> u32 {
    assert!(offset % 8 == 0, "Offset must be a multiple of 8");
    assert!(offset <= 32760, "Offset must be <= 32760");

    let imm12 = (offset / 8) as u32;
    // LDR (immediate, unsigned offset): 1111100101imm12nnnnnddddd
    // size=11 (64-bit), V=0, opc=01
    0b11111001_01 << 22 | (imm12 & 0xFFF) << 10 | (rn & 0x1F) << 5 | (rd & 0x1F)
}

fn build_add(rd: u32, rn: u32, rm: u32) -> u32 {
    // ADD (shifted register): 10001011000mmmmm000000nnnnnddddd
    0b10001011_00 << 22 | (rm & 0x1F) << 16 | (rn & 0x1F) << 5 | (rd & 0x1F)
}

fn load_field_from_register(
    dest_reg: u32,
    base_reg: u32,
    offset: u64,
) -> Instructions {
    let mut instructions = Instructions::new();

    if offset == 0 {
        // オフセットが0の場合、直接読み込み
        instructions.push(build_ldr_offset(dest_reg, base_reg, 0));
    } else if offset % 8 == 0 && offset <= 32760 {
        // オフセットが即値範囲内の場合、LDRの即値オフセットを使用
        instructions.push(build_ldr_offset(dest_reg, base_reg, offset));
    } else {
        // オフセットが大きい場合、レジスタ経由で計算
        // 一時レジスタを選択（dest_regとbase_regを避ける）
        let temp_reg = if dest_reg != 1 && base_reg != 1 {
            1
        } else if dest_reg != 2 && base_reg != 2 {
            2
        } else {
            3
        };

        // temp = base (ベースレジスタを保存)
        instructions.push(build_mov(temp_reg, base_reg));
        // dest = offset
        instructions.join(build_large_mov(dest_reg, offset));
        // dest = temp + dest (アドレス計算)
        instructions.push(build_add(dest_reg, temp_reg, dest_reg));
        // dest = *dest (フィールド値を読み込み)
        instructions.push(build_ldr_offset(dest_reg, dest_reg, 0));
    }

    instructions
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

#[test]
fn build_mov_test() {
    // mov x0, x28 (実際は orr x0, xzr, x28)
    assert_eq!(build_mov(0, 28), 0xaa1c03e0);
    // mov x1, x28
    assert_eq!(build_mov(1, 28), 0xaa1c03e1);
    // mov x2, x3
    assert_eq!(build_mov(2, 3), 0xaa0303e2);
}

#[test]
fn build_ldr_offset_test() {
    // ldr x0, [x28] (offset=0)
    assert_eq!(build_ldr_offset(0, 28, 0), 0xf9400380);
    // ldr x0, [x0] (offset=0)
    assert_eq!(build_ldr_offset(0, 0, 0), 0xf9400000);
    // ldr x0, [x28, #152] (goid offset)
    assert_eq!(build_ldr_offset(0, 28, 152), 0xf9404f80);
}

#[test]
fn build_add_test() {
    // add x0, x1, x0
    assert_eq!(build_add(0, 1, 0), 0x8b000020);
    // add x0, x28, x0
    assert_eq!(build_add(0, 28, 0), 0x8b000380);
}

#[test]
fn test_load_field_from_register() {
    // Test offset == 0: ldr x0, [x28]
    let inst = load_field_from_register(0, 28, 0);
    assert_eq!(inst.get(0), Some(0xf9400380)); // ldr x0, [x28]

    // Test offset == 152 (goid): ldr x0, [x28, #152]
    let inst = load_field_from_register(0, 28, 152);
    assert_eq!(inst.get(0), Some(0xf9404f80)); // ldr x0, [x28, #152]

    // Test different destination register: ldr x2, [x28, #152]
    let inst = load_field_from_register(2, 28, 152);
    assert_eq!(inst.get(0), Some(0xf9404f82)); // ldr x2, [x28, #152]

    // Test large offset (requires register calculation)
    let inst = load_field_from_register(0, 28, 40000);
    // Should generate: mov x1, x28; movz x0, ...; add x0, x1, x0; ldr x0, [x0]
    assert_eq!(inst.get(0), Some(build_mov(1, 28)));
    // The rest depends on build_large_mov implementation
}

pub trait TrampolineBuilder {
    fn build(
        &self,
        buffer_addr: u64,
        replaced_addr: u64,
        inst1: u32,
        inst2: u32,
        inst3: u32,
        inst4: u32,
        event_type: u8,
        runtime_offsets: &crate::dwarf::RuntimeOffsets,
    ) -> Instructions;
}

pub struct EntryTrampolineBuilder();

impl TrampolineBuilder for EntryTrampolineBuilder {
    fn build(
        &self,
        buffer_addr: u64,
        replaced_addr: u64,
        inst1: u32,
        inst2: u32,
        inst3: u32,
        inst4: u32,
        event_type: u8,
        runtime_offsets: &crate::dwarf::RuntimeOffsets,
    ) -> Instructions {
        let mut instructions = Instructions::new();
        instructions.join(push_registers_to_stack());

        // スタック確保: sub sp, sp, #32 (24バイト構造体 + 8バイトアライメント)
        instructions.push(0xd10083ff); // sub sp, sp, #32

        // event_type (1バイト) + padding (7バイト) を [sp] に書き込み
        // x0にevent_typeを設定し、8バイト書き込み（残り7バイトは自動的に0）
        instructions.push(build_movz(0, event_type as u32, 0));
        instructions.push(0xf90003e0); // str x0, [sp] - 8バイト書き込み

        // x28（goroutineポインタ）からgoidを読み取り、x0に格納後、[sp+8]に保存
        // x28はAArch64のGo runtimeでgoroutineポインタとして使用される
        instructions.join(load_field_from_register(0, 28, runtime_offsets.goid));
        instructions.push(0xf90007e0); // str x0, [sp, #8]

        // タイムスタンプを [sp+16] に保存
        instructions.push(0xd53be040); // mrs x0, cntvct_el0
        instructions.push(0xf9000be0); // str x0, [sp, #16]

        // x9 = buffer_addr
        instructions.join(build_large_mov(9, buffer_addr));
        // x10 = write_pos (Load-Acquire)
        instructions.push(0xc8dffd2a); // ldar x10, [x9]
        // x11 = idx = write_pos & 127
        instructions.push(0x9240194b); // and x11, x10, #127
        // x12 = idx * 24 = idx * 16 + idx * 8
        instructions.push(0xd37ced6c); // lsl x12, x11, #4
        instructions.push(0x8b0b0d8c); // add x12, x12, x11, lsl #3
        // x12 = buffer_addr + 64 + idx * 24
        instructions.push(0x8b09018c); // add x12, x12, x9
        instructions.push(0x9101018c); // add x12, x12, #64
        // TraceEvent (24バイト) をスタックから書き込み
        instructions.push(0xf94003ed); // ldr x13, [sp]
        instructions.push(0xf900018d); // str x13, [x12]
        instructions.push(0xf94007ed); // ldr x13, [sp, #8]
        instructions.push(0xf900058d); // str x13, [x12, #8]
        instructions.push(0xf9400bed); // ldr x13, [sp, #16]
        instructions.push(0xf900098d); // str x13, [x12, #16]
        // write_pos++ (Store-Release)
        instructions.push(0x9100054a); // add x10, x10, #1
        instructions.push(0xc89ffd2a); // stlr x10, [x9]

        // スタッククリーンアップ: add sp, sp, #32
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
}

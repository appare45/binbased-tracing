use super::Instructions;

pub(in crate::instruction) fn build_stp(rt: u32, rt2: u32, rn: u32, imm: i32) -> u32 {
    let imm7 = ((imm / 8) as u32) & 0x7F;
    0b1010_1001_10 << 22 | imm7 << 15 | (rt2 & 0x1F) << 10 | (rn & 0x1F) << 5 | (rt & 0x1F)
}

pub(in crate::instruction) fn build_ldp(rt: u32, rt2: u32, rn: u32, imm: i32) -> u32 {
    let imm7 = ((imm / 8) as u32) & 0x7F;
    0b1010_1000_11 << 22 | imm7 << 15 | (rt2 & 0x1F) << 10 | (rn & 0x1F) << 5 | (rt & 0x1F)
}

pub(in crate::instruction) fn build_movz(target: u32, value: u32, shift: u32) -> u32 {
    0b110100101 << 23 | ((shift / 16) & 0b11) << 21 | ((value & 0xFFFF) << 5) | (target & 0x1F)
}

pub(in crate::instruction) fn build_movk(target: u32, value: u32, shift: u32) -> u32 {
    0b111100101 << 23 | ((shift / 16) & 0b11) << 21 | ((value & 0xFFFF) << 5) | (target & 0x1F)
}

pub(in crate::instruction) fn build_mov(rd: u32, rn: u32) -> u32 {
    // orr xd, xzr, xn
    0b10101010_00 << 22 | (rn & 0x1F) << 16 | 0b11111 << 5 | (rd & 0x1F)
}

pub(in crate::instruction) fn build_ldr_offset(rd: u32, rn: u32, offset: u64) -> u32 {
    assert!(offset % 8 == 0, "Offset must be a multiple of 8");
    assert!(offset <= 32760, "Offset must be <= 32760");

    let imm12 = (offset / 8) as u32;
    0b11111001_01 << 22 | (imm12 & 0xFFF) << 10 | (rn & 0x1F) << 5 | (rd & 0x1F)
}

pub(in crate::instruction) fn build_add(rd: u32, rn: u32, rm: u32) -> u32 {
    // ADD (shifted register): 10001011000mmmmm000000nnnnnddddd
    0b10001011_00 << 22 | (rm & 0x1F) << 16 | (rn & 0x1F) << 5 | (rd & 0x1F)
}

pub(in crate::instruction) fn build_large_mov(target: u32, value: u64) -> Instructions {
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

pub(in crate::instruction) fn load_field_from_register(dest_reg: u32, base_reg: u32, offset: u64) -> Instructions {
    let mut instructions = Instructions::new();

    if offset == 0 {
        instructions.push(build_ldr_offset(dest_reg, base_reg, 0));
    } else if offset % 8 == 0 && offset <= 32760 {
        instructions.push(build_ldr_offset(dest_reg, base_reg, offset));
    } else {
        let temp_reg = if dest_reg != 1 && base_reg != 1 {
            1
        } else if dest_reg != 2 && base_reg != 2 {
            2
        } else {
            3
        };

        instructions.push(build_mov(temp_reg, base_reg));
        instructions.join(build_large_mov(dest_reg, offset));
        instructions.push(build_add(dest_reg, temp_reg, dest_reg));
        instructions.push(build_ldr_offset(dest_reg, dest_reg, 0));
    }

    instructions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_stp_test() {
        assert_eq!(build_stp(0, 1, 31, -16), 0xa9bf07e0);
        assert_eq!(build_stp(16, 17, 31, -16), 0xa9bf47f0);
    }

    #[test]
    fn build_ldp_test() {
        assert_eq!(build_ldp(16, 17, 31, 16), 0xa8c147f0)
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
        assert_eq!(build_mov(0, 28), 0xaa1c03e0);
        assert_eq!(build_mov(1, 28), 0xaa1c03e1);
        assert_eq!(build_mov(2, 3), 0xaa0303e2);
    }

    #[test]
    fn build_ldr_offset_test() {
        assert_eq!(build_ldr_offset(0, 28, 0), 0xf9400380);
        assert_eq!(build_ldr_offset(0, 0, 0), 0xf9400000);
        assert_eq!(build_ldr_offset(0, 28, 152), 0xf9404f80);
    }

    #[test]
    fn build_add_test() {
        assert_eq!(build_add(0, 1, 0), 0x8b000020);
        assert_eq!(build_add(0, 28, 0), 0x8b000380);
    }

    #[test]
    fn test_load_field_from_register() {
        let inst = load_field_from_register(0, 28, 0);
        assert_eq!(inst.get(0), Some(0xf9400380));

        let inst = load_field_from_register(0, 28, 152);
        assert_eq!(inst.get(0), Some(0xf9404f80));

        let inst = load_field_from_register(2, 28, 152);
        assert_eq!(inst.get(0), Some(0xf9404f82));

        let inst = load_field_from_register(0, 28, 40000);
        assert_eq!(inst.get(0), Some(build_mov(1, 28)));
    }
}

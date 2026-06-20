use super::Instructions;
use trampoline_blob_common::{MAGIC_BUFFER_ADDR, MAGIC_GOID_OFFSET, MAGIC_HEADER_VAL};

static TRAMPOLINE_BLOB_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/trampoline.bin"));

const MOVZ_MOVK_IMM16_MASK: u32 = 0xFFFF << 5;

// blob末尾の `ret` はエピローグ生成側（BranchStrategy::build_epilogue）が
// 元コードへの分岐を動的に追記するため除外する
const RET_INSTRUCTION: u32 = 0xd65f03c0;

fn blob_words() -> Vec<u32> {
    let mut words: Vec<u32> = TRAMPOLINE_BLOB_BYTES
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    assert_eq!(
        words.pop(),
        Some(RET_INSTRUCTION),
        "trampoline blob must end with ret"
    );
    words
}

fn patch_magic_chunks(words: &mut [u32], magics: &[u16; 4], value: u64) {
    for (i, &magic) in magics.iter().enumerate() {
        let chunk = ((value >> (i * 16)) & 0xFFFF) as u32;
        let pos = words
            .iter()
            .position(|&w| (w >> 5) & 0xFFFF == magic as u32)
            .expect("magic placeholder not found in trampoline blob");
        words[pos] = (words[pos] & !MOVZ_MOVK_IMM16_MASK) | (chunk << 5);
    }
}

pub fn build_trampoline_from_blob(
    header_val: u64,
    goid_offset: u64,
    buffer_addr: u64,
) -> Instructions {
    let mut words = blob_words();
    // TODO: 構造体にしたい
    patch_magic_chunks(&mut words, &MAGIC_HEADER_VAL, header_val);
    patch_magic_chunks(&mut words, &MAGIC_GOID_OFFSET, goid_offset);
    patch_magic_chunks(&mut words, &MAGIC_BUFFER_ADDR, buffer_addr);

    let mut instructions = Instructions::new();
    for w in words {
        instructions.push(w);
    }
    instructions
}

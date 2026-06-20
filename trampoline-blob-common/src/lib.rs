#![no_std]

//! trampoline-blob（aarch64-unknown-none）とbinbased-tracing本体の間で
//! マジックナンバープレースホルダの値を共有するためのクレート。
//! trampoline-blob/src/main.rsのmovz/movk即値と
//! binbased-tracing/src/instruction/blob.rsのパッチ位置検索が常に一致するよう、
//! 値の定義箇所を一本化する。

// header_val: x0 = movz/movk x0, #chunk, lsl #(16*i) for i in 0..4
pub const MAGIC_HEADER_VAL: [u16; 4] = [0xAAA1, 0xAAA2, 0xAAA3, 0xAAA4];
// goid offset: x1 = movz/movk x1, #chunk, lsl #(16*i) for i in 0..4
pub const MAGIC_GOID_OFFSET: [u16; 4] = [0xBBB1, 0xBBB2, 0xBBB3, 0xBBB4];
// buffer_addr: x9 = movz/movk x9, #chunk, lsl #(16*i) for i in 0..4
pub const MAGIC_BUFFER_ADDR: [u16; 4] = [0xCCC1, 0xCCC2, 0xCCC3, 0xCCC4];

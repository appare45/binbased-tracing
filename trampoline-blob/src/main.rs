#![no_std]
#![no_main]

use core::arch::naked_asm;
use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

/// トランポリン本体。エピローグ（元コードへの分岐）は呼び出し元が動的に追記する。
///
/// レジスタ前提:
/// - x28: goroutineポインタ（Go runtimeのABI、呼び出し元では変更しない）
/// - sp: 呼び出し元のスタック
#[unsafe(naked)]
#[unsafe(no_mangle)]
pub extern "C" fn trampoline_body() {
    naked_asm!(
        // 全レジスタ退避 x0-x29 (STP x{2i},x{2i+1}, [sp,#-16]! を15回)
        "stp x0, x1, [sp, #-16]!",
        "stp x2, x3, [sp, #-16]!",
        "stp x4, x5, [sp, #-16]!",
        "stp x6, x7, [sp, #-16]!",
        "stp x8, x9, [sp, #-16]!",
        "stp x10, x11, [sp, #-16]!",
        "stp x12, x13, [sp, #-16]!",
        "stp x14, x15, [sp, #-16]!",
        "stp x16, x17, [sp, #-16]!",
        "stp x18, x19, [sp, #-16]!",
        "stp x20, x21, [sp, #-16]!",
        "stp x22, x23, [sp, #-16]!",
        "stp x24, x25, [sp, #-16]!",
        "stp x26, x27, [sp, #-16]!",
        "stp x28, x29, [sp, #-16]!",

        "sub sp, sp, #32",

        // header_val を x0 に構築 -> [sp, #0]
        "movz x0, #{h0}, lsl #0",
        "movk x0, #{h1}, lsl #16",
        "movk x0, #{h2}, lsl #32",
        "movk x0, #{h3}, lsl #48",
        "str x0, [sp]",

        // goid offset を x1 に構築 -> x1 = [x28 + offset] -> [sp, #8]
        "movz x1, #{g0}, lsl #0",
        "movk x1, #{g1}, lsl #16",
        "movk x1, #{g2}, lsl #32",
        "movk x1, #{g3}, lsl #48",
        "add x1, x28, x1",
        "ldr x1, [x1]",
        "str x1, [sp, #8]",

        // タイムスタンプ -> [sp, #16]
        "mrs x0, cntvct_el0",
        "str x0, [sp, #16]",

        // buffer_addr を x9 に構築
        "movz x9, #{b0}, lsl #0",
        "movk x9, #{b1}, lsl #16",
        "movk x9, #{b2}, lsl #32",
        "movk x9, #{b3}, lsl #48",

        // リングバッファ書き込み位置計算（write_pos % 128 として64バイト/エントリ、ヘッダ64バイト）
        "ldar x10, [x9]",
        "and x11, x10, #127",
        "lsl x12, x11, #4",
        "add x12, x12, x11, lsl #3",
        "add x12, x12, x9",
        "add x12, x12, #64",

        "ldr x13, [sp]",
        "str x13, [x12]",
        "ldr x13, [sp, #8]",
        "str x13, [x12, #8]",
        "ldr x13, [sp, #16]",
        "str x13, [x12, #16]",

        "add x10, x10, #1",
        "stlr x10, [x9]",

        "add sp, sp, #32",

        // レジスタ復元 x29-x0 (LDP逆順)
        "ldp x28, x29, [sp], #16",
        "ldp x26, x27, [sp], #16",
        "ldp x24, x25, [sp], #16",
        "ldp x22, x23, [sp], #16",
        "ldp x20, x21, [sp], #16",
        "ldp x18, x19, [sp], #16",
        "ldp x16, x17, [sp], #16",
        "ldp x14, x15, [sp], #16",
        "ldp x12, x13, [sp], #16",
        "ldp x10, x11, [sp], #16",
        "ldp x8, x9, [sp], #16",
        "ldp x6, x7, [sp], #16",
        "ldp x4, x5, [sp], #16",
        "ldp x2, x3, [sp], #16",
        "ldp x0, x1, [sp], #16",

        // blobの終端: エピローグはメインクレートが動的に追記する
        "ret",
        h0 = const trampoline_blob_common::MAGIC_HEADER_VAL[0],
        h1 = const trampoline_blob_common::MAGIC_HEADER_VAL[1],
        h2 = const trampoline_blob_common::MAGIC_HEADER_VAL[2],
        h3 = const trampoline_blob_common::MAGIC_HEADER_VAL[3],
        g0 = const trampoline_blob_common::MAGIC_GOID_OFFSET[0],
        g1 = const trampoline_blob_common::MAGIC_GOID_OFFSET[1],
        g2 = const trampoline_blob_common::MAGIC_GOID_OFFSET[2],
        g3 = const trampoline_blob_common::MAGIC_GOID_OFFSET[3],
        b0 = const trampoline_blob_common::MAGIC_BUFFER_ADDR[0],
        b1 = const trampoline_blob_common::MAGIC_BUFFER_ADDR[1],
        b2 = const trampoline_blob_common::MAGIC_BUFFER_ADDR[2],
        b3 = const trampoline_blob_common::MAGIC_BUFFER_ADDR[3],
    );
}

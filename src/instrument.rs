use std::os::raw::c_void;

use crate::{error::InstrumentError, ptrace};

pub enum Instrument {
    NotInstrumented(ptrace::Tracee),
    PreInstrumented(ptrace::Tracee),
    Instrumented(ptrace::Tracee),
}

impl From<ptrace::Tracee> for Instrument {
    fn from(value: ptrace::Tracee) -> Self {
        Instrument::NotInstrumented(value)
    }
}

impl Instrument {
    /**
     * トランポリンコードを置くための領域を確保する
     * */
    pub fn pre_instrument(self) -> Result<Instrument, InstrumentError> {
        match self {
            Instrument::NotInstrumented(tracee) => {
                // traceeをstopする
                tracee.stop()?;
                let tracee = tracee.wait()?;
                let regs = tracee.get_regs()?;
                let mut replaced = Vec::new();
                let instructions = build_mmap();
                for (i, b) in instructions.iter().enumerate() {
                    let addr = regs.pc + (i as u64) * 0x8;
                    let original = tracee.read(addr)?;
                    replaced.push(original);

                    tracee.write(addr, *b)?;
                }
                let tracee = tracee.cont()?;
                let tracee = tracee.wait()?;
                // 元の8バイト値を2命令分ずつ復元
                for (chunk_idx, original_value) in replaced.iter().enumerate() {
                    let addr = regs.pc + (chunk_idx as u64) * 0x8; // 8バイト単位でアドレス計算
                    tracee.write(addr, *original_value)?;
                }
                let mut current_regs = tracee.get_regs()?;
                current_regs.pc = regs.pc;
                tracee.set_regs(current_regs)?;
                Ok(Instrument::PreInstrumented(tracee))
            }
            _ => Err(InstrumentError::AlreadyPreInstrumented),
        }
    }
}

// 一旦Xと出力するシステムコールを呼ぶだけのコードを生成する
// 元のアセンブリはasm.sを参照
fn build_mmap() -> Vec<i64> {
    let mut buf = Vec::new();
    // as -o asm.o asm.s && objdump -d asm.o
    buf.push(0x52800820 | 0x381f0fe0 << 32);
    buf.push(0xd2800020 | 0x910003e1 << 32);
    buf.push(0xd2800022 | 0xd2800808 << 32);
    buf.push(0xd4000001 | 0x910043ff << 32);
    buf.push(0xd4200000);
    return buf;
}

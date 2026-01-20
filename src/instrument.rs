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
                let instructions = build_mmap();

                let tracee = tracee.stop()?;
                let regs = tracee.get_regs()?;
                let pc = regs.pc;
                let saved = tracee.write_instructions(pc, &instructions)?;

                let tracee = tracee.cont()?.wait()?;
                tracee.write_instructions(pc, &saved)?;
                tracee.set_regs(regs)?;

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

use crate::{error::InstrumentError, proc, ptrace};

pub enum Instrument {
    NotInstrumented(ptrace::Tracee),
    PreInstrumented(ptrace::Tracee, u64),
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
                let base = tracee.base().unwrap_or(0);
                println!("Base: {base:x}");
                let instructions = build_mmap();

                let tracee = tracee.stop()?;
                let saved_regs = tracee.get_regs()?;
                let pc = saved_regs.pc;
                let mut regs = saved_regs.clone();
                let params = [
                    base,
                    1024,     // Size
                    3,        // PROT_READ | PROT_WRITE
                    0x22,     // MAP_PRIVATE | MAP_ANONYMOUS
                    u64::MAX, // -1
                ];
                for (i, v) in params.iter().enumerate() {
                    regs.regs[i] = *v;
                }
                regs.regs[8] = 222;
                tracee.set_regs(regs)?;
                let saved = tracee.write(pc, &instructions)?;

                let tracee = tracee.cont()?.wait()?;
                let addr = tracee.get_regs()?.regs[0];
                println!("Allocated at 0x{addr:x}");
                tracee.write(pc, &saved)?;
                tracee.set_regs(saved_regs)?;

                Ok(Instrument::PreInstrumented(tracee, addr))
            }
            _ => Err(InstrumentError::AlreadyPreInstrumented),
        }
    }
}

// トランポリンコードを置くためのmmapを呼び出す
fn build_mmap() -> Vec<i64> {
    let mut buf = Vec::new();
    buf.push(0xd4000001u64 as i64 | 0xd4200000 << 32); // svc #0; brk #0
    return buf;
}

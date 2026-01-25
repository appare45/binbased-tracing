use crate::{error::InstrumentError, instruction, proc, ptrace};

pub enum Instrument {
    NotInstrumented(ptrace::Attached),
    PreInstrumented(ptrace::Stopped, u64),
    Instrumented(proc::Proc),
}

const TARGET_SYMBOL: &str = "net/http.serverHandler.ServeHTTP";

impl TryFrom<proc::Proc> for Instrument {
    type Error = InstrumentError;

    fn try_from(value: proc::Proc) -> Result<Self, Self::Error> {
        Ok(Instrument::NotInstrumented(value.try_into()?))
    }
}

const TRAMPOLINE_SIZE: u64 = 1024;

impl Instrument {
    /**
     * トランポリンコードを置くための領域を確保する
     * */
    pub fn pre_instrument(self) -> Result<Instrument, InstrumentError> {
        match self {
            Instrument::NotInstrumented(tracee) => {
                let instructions = build_svc();

                let tracee = ptrace::Stopped::try_from(tracee)?;
                let saved_regs = tracee.get_regs()?;
                let pc = saved_regs.pc;
                let mut regs = saved_regs.clone();
                let params = [
                    0,               // addr hint
                    TRAMPOLINE_SIZE, // Size
                    3,               // PROT_READ | PROT_WRITE
                    0x22,            // MAP_PRIVATE | MAP_ANONYMOUS
                    u64::MAX,        // -1
                ];
                for (i, v) in params.iter().enumerate() {
                    regs.regs[i] = *v;
                }
                regs.regs[8] = 222;
                tracee.set_regs(regs)?;
                let saved = tracee.write(pc, &instructions.into())?;

                let tracee = ptrace::Attached::try_from(tracee)?.wait()?;
                let result_regs = tracee.get_regs()?;
                let addr = result_regs.regs[0];
                println!("Allocated at 0x{addr:x}");
                if addr == 0 || (addr as i64) < 0 {
                    eprintln!("mmap failed! Return value: 0x{:x} ({})", addr, addr as i64);
                    eprintln!("Syscall might have failed. Check errno.");
                    return Err(InstrumentError::MmapFailed);
                }
                tracee.write(pc, &saved.into())?;
                tracee.set_regs(saved_regs)?;

                Ok(Instrument::PreInstrumented(tracee, addr))
            }
            _ => Err(InstrumentError::AlreadyPreInstrumented),
        }
    }

    pub fn instrument(self) -> Result<Instrument, InstrumentError> {
        match self {
            Instrument::PreInstrumented(tracee, trampoline_addr) => {
                // ターゲットアドレスを取得
                let elf = tracee.0.get_bin().unwrap();
                let exec_base = tracee.0.exe_base().unwrap();
                let (off, _size) = elf.get_symbol(TARGET_SYMBOL.into()).unwrap();
                let target_addr = off + exec_base;
                println!("{TARGET_SYMBOL} is at 0x{target_addr:x}");

                // 4バイト分ずらすので注意！
                let target_bin1 = instruction::Instructions::from(tracee.read(target_addr)?);
                let target_bin2 = instruction::Instructions::from(tracee.read(target_addr + 8)?);
                let inst1 = target_bin1.get(0).unwrap();
                let inst2 = target_bin1.get(1).unwrap();
                let inst3 = target_bin2.get(0).unwrap();
                let inst4 = target_bin2.get(1).unwrap();

                let trampoline =
                    instruction::build_trampoline(target_addr, inst1, inst2, inst3, inst4);
                tracee.write(trampoline_addr, &trampoline.into())?;
                let saved_regs = tracee.get_regs()?;
                let mut regs = saved_regs.clone();
                let pc = regs.pc;
                let args = [
                    trampoline_addr,
                    TRAMPOLINE_SIZE,
                    5, // PROT_READ | PROT_EXEC
                ];
                for (i, v) in args.iter().enumerate() {
                    regs.regs[i] = *v
                }
                regs.regs[8] = 226;
                tracee.set_regs(regs)?;
                let buf = build_svc();
                let before = tracee.write(pc, &buf.into())?;
                let tracee = ptrace::Attached::try_from(tracee)?.wait()?;
                match tracee.get_regs()?.regs[0] {
                    code if code != 0 => return Err(InstrumentError::MprotectFailed(code)),
                    _ => (),
                }
                tracee.write(pc, &before.into())?;
                tracee.set_regs(saved_regs)?;

                let patch = instruction::jump_to_abs(trampoline_addr);
                tracee.write(target_addr, &patch.into())?;

                Ok(Instrument::Instrumented(tracee.try_into()?))
            }
            _ => Err(InstrumentError::NotPreInstrumentd),
        }
    }
}

// システムコールを呼び出す
fn build_svc() -> instruction::Instructions {
    let mut instructions = instruction::Instructions::new();
    instructions.push(0xd4000001);
    instructions.push(0xd4200000); // svc #0; brk #0
    return instructions;
}

use crate::{error::InstrumentError, instruction, proc, ptrace};

pub enum Instrument {
    NotInstrumented(ptrace::Tracee),
    PreInstrumented(ptrace::Tracee, u64),
    Instrumented(proc::Proc),
}

const TARGET_SYMBOL: &str = "net/http.serverHandler.ServeHTTP";

impl From<ptrace::Tracee> for Instrument {
    fn from(value: ptrace::Tracee) -> Self {
        Instrument::NotInstrumented(value)
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
                let base = tracee.base().unwrap_or(0);
                println!("Base: {base:x}");
                let instructions = build_svc();

                let tracee = tracee.stop()?;
                let saved_regs = tracee.get_regs()?;
                let pc = saved_regs.pc;
                let mut regs = saved_regs.clone();
                let params = [
                    base,
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

    pub fn instrument(self) -> Result<Instrument, InstrumentError> {
        match self {
            Instrument::PreInstrumented(tracee, trampoline_addr) => {
                let trampoline = build_trampoline();
                tracee.write(trampoline_addr, &trampoline)?;
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
                let before = tracee.write(pc, &buf)?;
                let tracee = tracee.cont()?;
                let tracee = tracee.wait()?;
                match tracee.get_regs()?.regs[0] {
                    code if code != 0 => return Err(InstrumentError::MprotectFailed(code)),
                    _ => (),
                }
                tracee.write(pc, &before)?;
                tracee.set_regs(saved_regs)?;
                // TODO: このあたりを分割したい
                let elf = tracee.get_bin().unwrap();
                let exec_base = tracee.base().unwrap();
                let (off, _size) = elf.get_symbol(TARGET_SYMBOL.into()).unwrap();
                let target_addr = off + exec_base;
                println!("{TARGET_SYMBOL} is at 0x{target_addr:x}");
                let target_bin_head = tracee.read(target_addr)?;
                Ok(Instrument::Instrumented(tracee.detach()?))
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

fn build_trampoline() -> instruction::Instructions {
    let mut instructions = instruction::Instructions::new();
    instructions.join(push_registers_to_stack());
    instructions.push(0x52800820u32);
    instructions.push(0x381f0fe0u32);
    instructions.push(0xd2800020u32);
    instructions.push(0x910003e1u32);
    instructions.push(0xd2800022u32);
    instructions.push(0xd2800808u32);
    instructions.push(0xd4000001u32);
    instructions.push(0x910043ffu32);
    instructions.push(0xd65f03c0u32);
    return instructions;
}

fn push_registers_to_stack() -> instruction::Instructions {
    let mut instructions = instruction::Instructions::new();
    for i in 0..15 {
        let rt = i * 2;
        let rt2 = i * 2 + 1;
        let rn = 31; // sp is index 31 (0b11111)
        let imm7 = (16 * i) / 8; // Scale the offset by 8

        let instr = 0b1010100100 << 22
            | (imm7 & 0x7F) << 15
            | (rt2 & 0x1F) << 10
            | (rn & 0x1F) << 5
            | (rt & 0x1F);
        instructions.push(instr);
    }
    instructions.push(0xf9007bfe);
    instructions
}

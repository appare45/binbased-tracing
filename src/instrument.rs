use crate::{error::InstrumentError, instruction, pipe, proc, ptrace};
use std::ffi::CString;

const TRAMPOLINE_SIZE: u64 = 1024;

pub struct NotInstrumented {
    tracee: ptrace::Attached,
    target_addr: u64,
    pipe_path: String,
}

struct TrampolineAllocating {
    tracee: ptrace::Stopped,
    target_addr: u64,
    pipe_path: String,
}

struct TrampolineAllocated {
    tracee: ptrace::Attached,
    target_addr: u64,
    trampoline_exec_addr: u64,
    trampoline_stack_addr: u64,
}

struct TrampolineWriting {
    tracee: ptrace::Stopped,
    target_addr: u64,
    trampoline_addr: u64,
}

struct TrampolineWrote {
    tracee: ptrace::Attached,
    target_addr: u64,
    trampoline_addr: u64,
}

struct TrampolinePermissionChanging {
    tracee: ptrace::Stopped,
    target_addr: u64,
    trampoline_addr: u64,
}

struct TrampolinePermissionChanged {
    tracee: ptrace::Attached,
    target_addr: u64,
    trampoline_addr: u64,
}

pub fn new(
    value: proc::Proc,
    target_addr: u64,
    pipe: &pipe::Pipe,
) -> Result<NotInstrumented, InstrumentError> {
    let tracee = ptrace::Attached::try_from(value)?;
    Ok(NotInstrumented {
        tracee,
        target_addr,
        pipe_path: pipe.path().to_string(),
    })
}

impl NotInstrumented {
    pub fn instrument(self) -> Result<proc::Proc, InstrumentError> {
        // NotInstrumented → ... → Proc の完全な遷移
        let allocating = TrampolineAllocating::try_from(self)?;
        let allocated = TrampolineAllocated::try_from(allocating)?;
        let writing = TrampolineWriting::try_from(allocated)?;
        let wrote = TrampolineWrote::try_from(writing)?;
        let perm_changing = TrampolinePermissionChanging::try_from(wrote)?;
        let perm_changed = TrampolinePermissionChanged::try_from(perm_changing)?;
        let proc = proc::Proc::try_from(perm_changed)?;
        Ok(proc)
    }
}

impl TryFrom<NotInstrumented> for TrampolineAllocating {
    type Error = InstrumentError;

    fn try_from(value: NotInstrumented) -> Result<Self, Self::Error> {
        Ok(Self {
            tracee: ptrace::Stopped::try_from(value.tracee)?,
            target_addr: value.target_addr,
            pipe_path: value.pipe_path,
        })
    }
}

fn call_mmap(
    tracee: ptrace::Stopped,
    size: u64,
    prot: u64,
) -> Result<(ptrace::Attached, u64), InstrumentError> {
    let saved_regs = tracee.get_regs()?;
    let pc = saved_regs.pc;
    let mut regs = saved_regs.clone();

    // mmap引数設定
    let params = [
        0,        // addr hint
        size,     // Size
        prot,     // PROT_*
        0x22,     // MAP_PRIVATE | MAP_ANONYMOUS
        u64::MAX, // fd = -1
    ];
    for (i, v) in params.iter().enumerate() {
        regs.regs[i] = *v;
    }
    regs.regs[8] = 222; // mmap syscall number
    tracee.set_regs(regs)?;

    let saved = tracee.write_instructions(pc, build_svc())?;

    // システムコール実行
    let tracee = ptrace::Attached::try_from(tracee)?.wait()?;
    let result_regs = tracee.get_regs()?;
    let addr = result_regs.regs[0];

    println!("Allocated at 0x{addr:x}");
    if addr == 0 || (addr as i64) < 0 {
        eprintln!("mmap failed! Return value: 0x{:x} ({})", addr, addr as i64);
        eprintln!("Syscall might have failed. Check errno.");
        return Err(InstrumentError::MmapFailed);
    }

    // 元の状態に復元
    tracee.write_instructions(pc, saved)?;
    tracee.set_regs(saved_regs)?;

    Ok((ptrace::Attached::try_from(tracee)?, addr))
}

impl TryFrom<TrampolineAllocating> for TrampolineAllocated {
    type Error = InstrumentError;

    fn try_from(value: TrampolineAllocating) -> Result<Self, Self::Error> {
        let tracee = value.tracee;

        let (tracee, trampoline_exec_addr) = call_mmap(tracee, TRAMPOLINE_SIZE, 3)?; // PROT_READ | PROT_WRITE

        let tracee = ptrace::Stopped::try_from(tracee)?;

        // 別々で確保することで衝突しないようになっている
        let (tracee, trampoline_stack_addr) = call_mmap(tracee, TRAMPOLINE_SIZE, 3)?; // PROT_READ | PROT_WRITE

        let binding = CString::new(value.pipe_path.as_str())?;
        let fifo_path = binding.as_bytes_with_nul();

        let tracee = ptrace::Stopped::try_from(tracee)?;
        println!(
            "Writing pipe path to 0x{:x}: {:?}",
            trampoline_stack_addr, value.pipe_path
        );
        tracee.write_bytes(trampoline_stack_addr, fifo_path)?;
        println!(
            "Wrote {} bytes to 0x{:x}-0x{:x}",
            fifo_path.len(),
            trampoline_stack_addr,
            trampoline_stack_addr + fifo_path.len() as u64
        );

        Ok(TrampolineAllocated {
            tracee: tracee.try_into()?,
            target_addr: value.target_addr,
            trampoline_exec_addr,
            trampoline_stack_addr,
        })
    }
}

impl TryFrom<TrampolineAllocated> for TrampolineWriting {
    type Error = InstrumentError;

    fn try_from(value: TrampolineAllocated) -> Result<Self, Self::Error> {
        let tracee = ptrace::Stopped::try_from(value.tracee)?;
        let trampoline_stack_addr = value.trampoline_stack_addr;
        let trampoline_exe_addr = value.trampoline_exec_addr;
        let target_addr = value.target_addr;

        // 4バイト分ずらすので注意！
        let target_bin1 = instruction::Instructions::from(tracee.read(target_addr)?);
        let target_bin2 = instruction::Instructions::from(tracee.read(target_addr + 8)?);
        let inst1 = target_bin1.get(0).unwrap();
        let inst2 = target_bin1.get(1).unwrap();
        let inst3 = target_bin2.get(0).unwrap();
        let inst4 = target_bin2.get(1).unwrap();

        // トランポリンコードを構築して書き込み
        let trampoline = instruction::build_trampoline(
            trampoline_stack_addr,
            target_addr,
            inst1,
            inst2,
            inst3,
            inst4,
        );
        tracee.write_instructions(trampoline_exe_addr, trampoline)?;

        Ok(TrampolineWriting {
            tracee,
            target_addr,
            trampoline_addr: trampoline_exe_addr,
        })
    }
}

impl TryFrom<TrampolineWriting> for TrampolineWrote {
    type Error = InstrumentError;

    fn try_from(value: TrampolineWriting) -> Result<Self, Self::Error> {
        Ok(TrampolineWrote {
            tracee: ptrace::Attached::try_from(value.tracee)?,
            target_addr: value.target_addr,
            trampoline_addr: value.trampoline_addr,
        })
    }
}

impl TryFrom<TrampolineWrote> for TrampolinePermissionChanging {
    type Error = InstrumentError;

    fn try_from(value: TrampolineWrote) -> Result<Self, Self::Error> {
        Ok(TrampolinePermissionChanging {
            tracee: ptrace::Stopped::try_from(value.tracee)?,
            target_addr: value.target_addr,
            trampoline_addr: value.trampoline_addr,
        })
    }
}

impl TryFrom<TrampolinePermissionChanging> for TrampolinePermissionChanged {
    type Error = InstrumentError;

    fn try_from(value: TrampolinePermissionChanging) -> Result<Self, Self::Error> {
        let tracee = value.tracee;
        let trampoline_addr = value.trampoline_addr;
        let target_addr = value.target_addr;

        // mprotectシステムコールでトランポリンを実行可能にする
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
        regs.regs[8] = 226; // mprotect syscall number
        tracee.set_regs(regs)?;

        let buf = build_svc();
        let before = tracee.write_instructions(pc, buf)?;

        // システムコール実行
        let tracee = ptrace::Attached::try_from(tracee)?.wait()?;
        match tracee.get_regs()?.regs[0] {
            code if code != 0 => return Err(InstrumentError::MprotectFailed(code)),
            _ => (),
        }

        // 元の状態に復元
        tracee.write_instructions(pc, before)?;
        tracee.set_regs(saved_regs)?;

        Ok(TrampolinePermissionChanged {
            tracee: ptrace::Attached::try_from(tracee)?,
            target_addr,
            trampoline_addr: value.trampoline_addr,
        })
    }
}

impl TryFrom<TrampolinePermissionChanged> for proc::Proc {
    type Error = InstrumentError;

    fn try_from(value: TrampolinePermissionChanged) -> Result<Self, Self::Error> {
        let tracee = ptrace::Stopped::try_from(value.tracee)?;
        let trampoline_addr = value.trampoline_addr;
        let target_addr = value.target_addr;

        // ターゲットアドレスに絶対ジャンプを書き込み
        let patch = instruction::jump_to_abs(trampoline_addr);
        tracee.write_instructions(target_addr, patch)?;

        // デタッチして元のProcに戻す
        Ok(tracee.try_into()?)
    }
}

// システムコールを呼び出す
fn build_svc() -> instruction::Instructions {
    let mut instructions = instruction::Instructions::new();
    instructions.push(0xd4000001);
    instructions.push(0xd4200000); // svc #0; brk #0
    return instructions;
}

use crate::{error::InstrumentError, instruction, proc, ptrace};

const TRAMPOLINE_SIZE: u64 = 1024;
const TARGET_SYMBOL: &str = "net/http.serverHandler.ServeHTTP";

pub struct NotInstrumented(ptrace::Attached);
struct TrampolineAllocating(ptrace::Stopped);
struct TrampolineAllocated(ptrace::Attached, u64); // u64はトランポリンアドレス
struct TrampolineWriting(ptrace::Stopped, u64, u64); // (stopped, trampoline_addr, target_addr)
struct TrampolineWrote(ptrace::Attached, u64, u64); // (attached, trampoline_addr, target_addr)
struct TrampolinePermissionChanging(ptrace::Stopped, u64, u64);
struct TrampolinePermissionChanged(ptrace::Attached, u64, u64);

impl TryFrom<proc::Proc> for NotInstrumented {
    type Error = InstrumentError;

    fn try_from(value: proc::Proc) -> Result<Self, Self::Error> {
        let attached = ptrace::Attached::try_from(value)?;
        Ok(NotInstrumented(attached))
    }
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
        let stopped = ptrace::Stopped::try_from(value.0)?;
        Ok(TrampolineAllocating(stopped))
    }
}

impl TryFrom<TrampolineAllocating> for TrampolineAllocated {
    type Error = InstrumentError;

    fn try_from(value: TrampolineAllocating) -> Result<Self, Self::Error> {
        // mmapシステムコールを実行してトランポリン領域を確保
        let tracee = value.0;
        let saved_regs = tracee.get_regs()?;
        let pc = saved_regs.pc;
        let mut regs = saved_regs.clone();

        // mmap引数設定
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
        regs.regs[8] = 222; // mmap syscall number
        tracee.set_regs(regs)?;

        let saved = tracee.write(pc, &build_svc().into())?;

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
        tracee.write(pc, &saved.into())?;
        tracee.set_regs(saved_regs)?;

        Ok(TrampolineAllocated(
            ptrace::Attached::try_from(tracee)?,
            addr,
        ))
    }
}

impl TryFrom<TrampolineAllocated> for TrampolineWriting {
    type Error = InstrumentError;

    fn try_from(value: TrampolineAllocated) -> Result<Self, Self::Error> {
        let tracee = ptrace::Stopped::try_from(value.0)?;
        let trampoline_addr = value.1;

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

        // トランポリンコードを構築して書き込み
        let trampoline = instruction::build_trampoline(target_addr, inst1, inst2, inst3, inst4);
        tracee.write(trampoline_addr, &trampoline.into())?;

        Ok(TrampolineWriting(tracee, trampoline_addr, target_addr))
    }
}

impl TryFrom<TrampolineWriting> for TrampolineWrote {
    type Error = InstrumentError;

    fn try_from(value: TrampolineWriting) -> Result<Self, Self::Error> {
        let tracee = ptrace::Attached::try_from(value.0)?;
        Ok(TrampolineWrote(tracee, value.1, value.2))
    }
}

impl TryFrom<TrampolineWrote> for TrampolinePermissionChanging {
    type Error = InstrumentError;

    fn try_from(value: TrampolineWrote) -> Result<Self, Self::Error> {
        let tracee = ptrace::Stopped::try_from(value.0)?;
        Ok(TrampolinePermissionChanging(tracee, value.1, value.2))
    }
}

impl TryFrom<TrampolinePermissionChanging> for TrampolinePermissionChanged {
    type Error = InstrumentError;

    fn try_from(value: TrampolinePermissionChanging) -> Result<Self, Self::Error> {
        let tracee = value.0;
        let trampoline_addr = value.1;
        let target_addr = value.2;

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
        let before = tracee.write(pc, &buf.into())?;

        // システムコール実行
        let tracee = ptrace::Attached::try_from(tracee)?.wait()?;
        match tracee.get_regs()?.regs[0] {
            code if code != 0 => return Err(InstrumentError::MprotectFailed(code)),
            _ => (),
        }

        // 元の状態に復元
        tracee.write(pc, &before.into())?;
        tracee.set_regs(saved_regs)?;

        Ok(TrampolinePermissionChanged(
            ptrace::Attached::try_from(tracee)?,
            trampoline_addr,
            target_addr,
        ))
    }
}

impl TryFrom<TrampolinePermissionChanged> for proc::Proc {
    type Error = InstrumentError;

    fn try_from(value: TrampolinePermissionChanged) -> Result<Self, Self::Error> {
        let tracee = ptrace::Stopped::try_from(value.0)?;
        let trampoline_addr = value.1;
        let target_addr = value.2;

        // ターゲットアドレスに絶対ジャンプを書き込み
        let patch = instruction::jump_to_abs(trampoline_addr);
        tracee.write(target_addr, &patch.into())?;

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

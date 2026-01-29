use crate::{
    error::InstrumentError, event::TraceEvent, instruction, pipe, proc, ptrace, symbol_analyzer,
};
use std::ffi::CString;
use std::sync::mpsc::Sender;
use std::thread::JoinHandle;

const TRAMPOLINE_SIZE: u64 = 1024;
const SYSCALL_MMAP: u64 = 222;
const SYSCALL_MPROTECT: u64 = 226;
const SYSCALL_OPEN: u64 = 56;

pub struct InstrumentTarget {
    pub addr: u64,
    pub builder: Box<dyn instruction::TrampolineBuilder>,
    pub pipe_path: String,
    pub event_type: u8,
}

pub struct InstrumentationPlan {
    pub targets: Vec<InstrumentTarget>,
    pub pipes: Vec<pipe::Pipe>,
    pub readers: Vec<JoinHandle<u64>>,
}

pub fn plan_instrumentation(
    proc: &proc::Proc,
    analysis: &symbol_analyzer::FunctionAnalysis,
    symbol_name: &str,
    event_tx: Sender<TraceEvent>,
) -> Result<InstrumentationPlan, InstrumentError> {
    let pipe_entry = pipe::Pipe::new(symbol_name, proc.pid, Some("entry"))?;
    let reader_entry = pipe_entry.start_reader(event_tx.clone());

    let pipe_end = pipe::Pipe::new(symbol_name, proc.pid, Some("end"))?;
    let reader_end = pipe_end.start_reader(event_tx);

    let mut targets = vec![InstrumentTarget {
        addr: analysis.entry_addr,
        builder: Box::new(instruction::EntryTrampolineBuilder()),
        pipe_path: pipe_entry.path().to_string(),
        event_type: 0, // Entry
    }];

    for ret_addr in &analysis.ret_addrs {
        targets.push(InstrumentTarget {
            addr: *ret_addr,
            builder: Box::new(instruction::EntryTrampolineBuilder()),
            pipe_path: pipe_end.path().to_string(),
            event_type: 1, // Return
        });
    }

    Ok(InstrumentationPlan {
        targets,
        pipes: vec![pipe_entry, pipe_end],
        readers: vec![reader_entry, reader_end],
    })
}

pub struct NotInstrumented {
    tracee: ptrace::Attached,
    targets: Vec<InstrumentTarget>,
}

struct AllocatedTarget {
    addr: u64,
    trampoline_addr: u64,
    pipe_fd: u32,
    builder: Box<dyn instruction::TrampolineBuilder>,
    event_type: u8,
}

struct TrampolineAllocating {
    tracee: ptrace::Stopped,
    targets: Vec<InstrumentTarget>,
}

struct TrampolineAllocated {
    tracee: ptrace::Attached,
    targets: Vec<AllocatedTarget>,
}

struct TrampolineWriting {
    tracee: ptrace::Stopped,
    targets: Vec<AllocatedTarget>,
}

struct TrampolineWrote {
    tracee: ptrace::Attached,
    targets: Vec<AllocatedTarget>,
}

struct TrampolinePermissionChanging {
    tracee: ptrace::Stopped,
    targets: Vec<AllocatedTarget>,
}

struct TrampolinePermissionChanged {
    tracee: ptrace::Attached,
    targets: Vec<AllocatedTarget>,
}

pub fn new(
    value: proc::Proc,
    targets: Vec<InstrumentTarget>,
) -> Result<NotInstrumented, InstrumentError> {
    let tracee = ptrace::Attached::try_from(value)?;
    Ok(NotInstrumented { tracee, targets })
}

impl NotInstrumented {
    pub fn instrument(self) -> Result<proc::Proc, InstrumentError> {
        // NotInstrumented → ... → Proc の遷移
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
            targets: value.targets,
        })
    }
}

// システムコールをptraceで呼ぶ
fn call_svc(
    tracee: ptrace::Stopped,
    syscall_number: u64,
    params: &[u64],
) -> Result<(ptrace::Attached, u64), InstrumentError> {
    let saved_regs = tracee.get_regs()?;
    let pc = saved_regs.pc;
    let mut regs = saved_regs.clone();

    // 引数設定
    for (i, v) in params.iter().enumerate() {
        if regs.regs.len() < i {
            break;
        }
        regs.regs[i] = *v;
    }
    regs.regs[8] = syscall_number;
    tracee.set_regs(regs)?;

    let saved = tracee.write_instructions(pc, build_svc())?;

    // システムコール実行
    let tracee = ptrace::Attached::try_from(tracee)?.wait()?;
    let result_regs = tracee.get_regs()?;
    let addr = result_regs.regs[0];

    if (addr as i64) < 0 {
        eprintln!(
            "syscall failed! Return value: 0x{:x} ({})",
            addr, addr as i64
        );
        return Err(InstrumentError::SyscallFailed(addr));
    }

    // 元の状態に復元
    tracee.write_instructions(pc, saved)?;
    tracee.set_regs(saved_regs)?;

    Ok((ptrace::Attached::try_from(tracee)?, addr))
}

impl TryFrom<TrampolineAllocating> for TrampolineAllocated {
    type Error = InstrumentError;

    fn try_from(value: TrampolineAllocating) -> Result<Self, Self::Error> {
        let mut tracee = value.tracee;

        // 適当なデータを置くための領域（ヒープ的な）
        let (attached, trampoline_stack_addr) = call_svc(
            tracee,
            SYSCALL_MMAP,
            &[
                0,               // addr hint
                TRAMPOLINE_SIZE, // Size
                3,               // PROT_*
                0x22,            // MAP_PRIVATE | MAP_ANONYMOUS
                u64::MAX,        // fd = -1
            ],
        )?;
        tracee = ptrace::Stopped::try_from(attached)?;

        let mut allocated_targets = Vec::new();

        for target in value.targets {
            let (attached, trampoline_addr) = call_svc(
                tracee,
                SYSCALL_MMAP,
                &[
                    0,               // addr hint
                    TRAMPOLINE_SIZE, // Size
                    3,               // PROT_*
                    0x22,            // MAP_PRIVATE | MAP_ANONYMOUS
                    u64::MAX,        // fd = -1
                ],
            )?;
            tracee = ptrace::Stopped::try_from(attached)?;

            let binding = CString::new(target.pipe_path.as_str())?;
            let pipe_path = binding.as_bytes_with_nul();
            println!(
                "Writing pipe path to 0x{:x}: {:?}",
                trampoline_stack_addr, target.pipe_path
            );
            tracee.write_bytes(trampoline_stack_addr, pipe_path)?;
            println!(
                "Wrote {} bytes to 0x{:x}-0x{:x}",
                pipe_path.len(),
                trampoline_stack_addr,
                trampoline_stack_addr + pipe_path.len() as u64
            );

            let (attached, pipe_fd) = call_svc(
                tracee,
                SYSCALL_OPEN,
                &[0xFFFFFFFFFFFFFF9C, trampoline_stack_addr, 0x801],
            )?;
            tracee = ptrace::Stopped::try_from(attached)?;

            allocated_targets.push(AllocatedTarget {
                addr: target.addr,
                trampoline_addr,
                pipe_fd: pipe_fd.try_into()?,
                builder: target.builder,
                event_type: target.event_type,
            });
        }

        Ok(TrampolineAllocated {
            tracee: tracee.try_into()?,
            targets: allocated_targets,
        })
    }
}

impl TryFrom<TrampolineAllocated> for TrampolineWriting {
    type Error = InstrumentError;

    fn try_from(value: TrampolineAllocated) -> Result<Self, Self::Error> {
        let tracee = ptrace::Stopped::try_from(value.tracee)?;

        for target in &value.targets {
            // 4バイト分ずらすので注意！
            let target_bin1 = instruction::Instructions::from(tracee.read(target.addr)?);
            let target_bin2 = instruction::Instructions::from(tracee.read(target.addr + 8)?);
            let inst1 = target_bin1.get(0).unwrap();
            let inst2 = target_bin1.get(1).unwrap();
            let inst3 = target_bin2.get(0).unwrap();
            let inst4 = target_bin2.get(1).unwrap();

            // トランポリンコードを構築して書き込み
            let trampoline = target.builder.build(
                target.pipe_fd,
                target.addr,
                inst1,
                inst2,
                inst3,
                inst4,
                target.event_type,
            );
            tracee.write_instructions(target.trampoline_addr, trampoline)?;
        }

        Ok(TrampolineWriting {
            tracee,
            targets: value.targets,
        })
    }
}

impl TryFrom<TrampolineWriting> for TrampolineWrote {
    type Error = InstrumentError;

    fn try_from(value: TrampolineWriting) -> Result<Self, Self::Error> {
        Ok(TrampolineWrote {
            tracee: ptrace::Attached::try_from(value.tracee)?,
            targets: value.targets,
        })
    }
}

impl TryFrom<TrampolineWrote> for TrampolinePermissionChanging {
    type Error = InstrumentError;

    fn try_from(value: TrampolineWrote) -> Result<Self, Self::Error> {
        Ok(TrampolinePermissionChanging {
            tracee: ptrace::Stopped::try_from(value.tracee)?,
            targets: value.targets,
        })
    }
}

impl TryFrom<TrampolinePermissionChanging> for TrampolinePermissionChanged {
    type Error = InstrumentError;

    fn try_from(value: TrampolinePermissionChanging) -> Result<Self, Self::Error> {
        let mut tracee = value.tracee;

        for target in &value.targets {
            // mprotectシステムコールでトランポリンを実行可能にする
            let (attached, result) = call_svc(
                tracee,
                SYSCALL_MPROTECT, // mprotect syscall number
                &[
                    target.trampoline_addr,
                    TRAMPOLINE_SIZE,
                    5, // PROT_READ | PROT_EXEC
                ],
            )?;

            if result != 0 {
                return Err(InstrumentError::MprotectFailed(result));
            }

            tracee = ptrace::Stopped::try_from(attached)?;
        }

        Ok(TrampolinePermissionChanged {
            tracee: ptrace::Attached::try_from(tracee)?,
            targets: value.targets,
        })
    }
}

impl TryFrom<TrampolinePermissionChanged> for proc::Proc {
    type Error = InstrumentError;

    fn try_from(value: TrampolinePermissionChanged) -> Result<Self, Self::Error> {
        let tracee = ptrace::Stopped::try_from(value.tracee)?;

        for target in &value.targets {
            // ターゲットアドレスに絶対ジャンプを書き込み
            let patch = instruction::jump_to_abs(target.trampoline_addr);
            tracee.write_instructions(target.addr, patch)?;
        }

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

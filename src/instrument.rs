use crate::{
    error::InstrumentError, event::TraceEvent, event_buffer::EventBuffer, instruction, proc,
    ptrace, symbol_analyzer,
};
use std::sync::mpsc::Sender;

const TRAMPOLINE_SIZE: u64 = 1024;
const SYSCALL_MMAP: u64 = 222;
const SYSCALL_MPROTECT: u64 = 226;

pub struct InstrumentTarget {
    pub addr: u64,
    pub builder: Box<dyn instruction::TrampolineBuilder>,
    pub buffer_fd: std::os::unix::io::RawFd,
    pub event_type: u8,
}

pub struct InstrumentationPlan {
    pub targets: Vec<InstrumentTarget>,
    pub buffers: Vec<EventBuffer>,
    pub runtime_offsets: crate::dwarf::RuntimeOffsets,
}

pub fn plan_instrumentation(
    proc: &proc::Proc,
    analysis: &symbol_analyzer::FunctionAnalysis,
    _symbol_name: &str,
    mut buffers: Vec<EventBuffer>,
    event_tx: Sender<TraceEvent>,
) -> Result<InstrumentationPlan, InstrumentError> {
    let exe_path = proc.exe_path()?;
    let elf_bytes = std::fs::read(&exe_path)?;
    let elf = crate::elf::new(&elf_bytes)?;

    let runtime_offsets = elf.runtime_offsets.ok_or(InstrumentError::DwarfError(
        crate::error::DwarfError::NoDwarfInfo,
    ))?;

    // buffers[0] = entry, buffers[1..] = ret命令（順に割り当て）
    // child_buffer_addr は instrument フェーズで ptrace 経由の mmap により決定する
    buffers[0].start_reader(event_tx.clone());

    let mut targets = vec![InstrumentTarget {
        addr: analysis.entry_addr,
        builder: Box::new(instruction::EntryTrampolineBuilder()),
        buffer_fd: buffers[0].fd(),
        event_type: 0,
    }];

    for (i, ret_addr) in analysis.ret_addrs.iter().enumerate() {
        let buf_idx = 1 + i;
        buffers[buf_idx].start_reader(event_tx.clone());
        targets.push(InstrumentTarget {
            addr: *ret_addr,
            builder: Box::new(instruction::EntryTrampolineBuilder()),
            buffer_fd: buffers[buf_idx].fd(),
            event_type: 1,
        });
    }

    Ok(InstrumentationPlan {
        targets,
        buffers,
        runtime_offsets,
    })
}

pub struct NotInstrumented {
    tracee: ptrace::Attached,
    targets: Vec<InstrumentTarget>,
    runtime_offsets: crate::dwarf::RuntimeOffsets,
}

struct AllocatedTarget {
    addr: u64,
    trampoline_addr: u64,
    child_buffer_addr: u64,
    builder: Box<dyn instruction::TrampolineBuilder>,
    event_type: u8,
}

struct TrampolineAllocating {
    tracee: ptrace::Stopped,
    targets: Vec<InstrumentTarget>,
    runtime_offsets: crate::dwarf::RuntimeOffsets,
}

struct TrampolineAllocated {
    tracee: ptrace::Attached,
    targets: Vec<AllocatedTarget>,
    runtime_offsets: crate::dwarf::RuntimeOffsets,
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
    runtime_offsets: crate::dwarf::RuntimeOffsets,
) -> Result<NotInstrumented, InstrumentError> {
    let tracee = ptrace::Attached::try_from(value)?;
    Ok(NotInstrumented {
        tracee,
        targets,
        runtime_offsets,
    })
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
            runtime_offsets: value.runtime_offsets,
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
        if regs.regs.len() <= i {
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

        let mut allocated_targets = Vec::new();

        for target in value.targets {
            let (attached, trampoline_addr) = call_svc(
                tracee,
                SYSCALL_MMAP,
                &[
                    0,               // addr hint
                    TRAMPOLINE_SIZE, // size
                    3,               // PROT_READ | PROT_WRITE
                    0x22,            // MAP_PRIVATE | MAP_ANONYMOUS
                    u64::MAX,        // fd = -1
                ],
            )?;
            tracee = ptrace::Stopped::try_from(attached)?;

            // バッファ用 mmap を子プロセス内で実行（MAP_SHARED でバッファ fd をマップ）
            const BUFFER_SIZE: u64 = 4096;
            let (attached, child_buffer_addr) = call_svc(
                tracee,
                SYSCALL_MMAP,
                &[
                    0,                        // addr hint
                    BUFFER_SIZE,              // size
                    3,                        // PROT_READ | PROT_WRITE
                    0x01,                     // MAP_SHARED
                    target.buffer_fd as u64,  // memfd fd
                    0,                        // offset
                ],
            )?;
            tracee = ptrace::Stopped::try_from(attached)?;

            allocated_targets.push(AllocatedTarget {
                addr: target.addr,
                trampoline_addr,
                child_buffer_addr,
                builder: target.builder,
                event_type: target.event_type,
            });
        }

        Ok(TrampolineAllocated {
            tracee: tracee.try_into()?,
            targets: allocated_targets,
            runtime_offsets: value.runtime_offsets,
        })
    }
}

impl TryFrom<TrampolineAllocated> for TrampolineWriting {
    type Error = InstrumentError;

    fn try_from(value: TrampolineAllocated) -> Result<Self, Self::Error> {
        let tracee = ptrace::Stopped::try_from(value.tracee)?;
        let runtime_offsets = &value.runtime_offsets;

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
                target.child_buffer_addr,
                target.addr,
                inst1,
                inst2,
                inst3,
                inst4,
                target.event_type,
                runtime_offsets,
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

use crate::{
    error::InstrumentError,
    instruction::{self, BranchStrategy},
    proc,
    ptrace,
};

use super::plan::{AllocatedTarget, InstrumentTarget, TRAMPOLINE_SIZE};

const SYSCALL_MMAP: u64 = 222;
const SYSCALL_MPROTECT: u64 = 226;

pub struct NotInstrumented {
    pub(super) tracee: ptrace::Attached,
    pub(super) targets: Vec<InstrumentTarget>,
    pub(super) runtime_offsets: crate::dwarf::RuntimeOffsets,
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

impl NotInstrumented {
    pub fn instrument(self) -> Result<proc::Proc, InstrumentError> {
        let allocating = TrampolineAllocating::try_from(self)?;
        let allocated = TrampolineAllocated::try_from(allocating)?;
        let writing = TrampolineWriting::try_from(allocated)?;
        let wrote = TrampolineWrote::try_from(writing)?;
        let perm_changing = TrampolinePermissionChanging::try_from(wrote)?;
        let perm_changed = TrampolinePermissionChanged::try_from(perm_changing)?;
        Ok(proc::Proc::try_from(perm_changed)?)
    }
}

impl TryFrom<NotInstrumented> for TrampolineAllocating {
    type Error = InstrumentError;

    fn try_from(value: NotInstrumented) -> Result<Self, Self::Error> {
        Ok(TrampolineAllocating {
            tracee: ptrace::Stopped::try_from(value.tracee)?,
            targets: value.targets,
            runtime_offsets: value.runtime_offsets,
        })
    }
}

fn call_svc(
    tracee: ptrace::Stopped,
    syscall_number: u64,
    params: &[u64],
) -> Result<(ptrace::Attached, u64), InstrumentError> {
    let saved_regs = tracee.get_regs()?;
    let pc = saved_regs.pc;
    let mut regs = saved_regs.clone();

    for (i, v) in params.iter().enumerate() {
        if regs.regs.len() <= i {
            break;
        }
        regs.regs[i] = *v;
    }
    regs.regs[8] = syscall_number;
    tracee.set_regs(regs)?;

    let saved = tracee.write_instructions(pc, build_svc())?;

    let tracee = ptrace::Attached::try_from(tracee)?.wait()?;
    let result_regs = tracee.get_regs()?;
    let addr = result_regs.regs[0];

    if (addr as i64) < 0 {
        eprintln!("syscall failed! Return value: 0x{:x} ({})", addr, addr as i64);
        return Err(InstrumentError::SyscallFailed(addr));
    }

    tracee.write_instructions(pc, saved)?;
    tracee.set_regs(saved_regs)?;

    Ok((ptrace::Attached::try_from(tracee)?, addr))
}

fn build_svc() -> instruction::Instructions {
    let mut instructions = instruction::Instructions::new();
    instructions.push(0xd4000001);
    instructions.push(0xd4200000);
    instructions
}

impl TryFrom<TrampolineAllocating> for TrampolineAllocated {
    type Error = InstrumentError;

    fn try_from(value: TrampolineAllocating) -> Result<Self, Self::Error> {
        let mut tracee = value.tracee;
        let mut allocated_targets = Vec::new();

        for target in value.targets {
            let hint_addr = target.trampoline_hint;
            eprintln!("target.addr={:#x} hint_addr={:#x}", target.addr, hint_addr);
            let (attached, trampoline_addr) = call_svc(
                tracee,
                SYSCALL_MMAP,
                &[hint_addr, TRAMPOLINE_SIZE, 3, 0x22, u64::MAX],
            )?;
            tracee = ptrace::Stopped::try_from(attached)?;

            const BUFFER_SIZE: u64 = 4096;
            let (attached, child_buffer_addr) = call_svc(
                tracee,
                SYSCALL_MMAP,
                &[0, BUFFER_SIZE, 3, 0x01, target.buffer.fd() as u64, 0],
            )?;
            tracee = ptrace::Stopped::try_from(attached)?;

            let patch = BranchStrategy::new(target.addr, trampoline_addr);
            allocated_targets.push(AllocatedTarget {
                patch,
                trampoline_addr,
                child_buffer_addr,
                event_type: target.event_type,
                target_id: target.target_id,
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
            let patch_addr = target.patch.patch_addr();
            let target_bin1 = instruction::Instructions::from(tracee.read(patch_addr)?);
            let target_bin2 = instruction::Instructions::from(tracee.read(patch_addr + 8)?);
            let inst1 = target_bin1.get(0).unwrap();
            let inst2 = target_bin1.get(1).unwrap();
            let inst3 = target_bin2.get(0).unwrap();
            let inst4 = target_bin2.get(1).unwrap();

            let trampoline = instruction::build_trampoline(
                &target.patch,
                target.child_buffer_addr,
                patch_addr,
                target.trampoline_addr,
                inst1, inst2, inst3, inst4,
                target.event_type,
                target.target_id,
                runtime_offsets,
            );
            tracee.write_instructions(target.trampoline_addr, trampoline)?;
        }

        Ok(TrampolineWriting { tracee, targets: value.targets })
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
            let (attached, result) = call_svc(
                tracee,
                SYSCALL_MPROTECT,
                &[target.trampoline_addr, TRAMPOLINE_SIZE, 5],
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
            let (instructions, write_addr) = target.patch.apply();
            tracee.write_instructions(write_addr, instructions)?;
        }

        Ok(tracee.try_into()?)
    }
}

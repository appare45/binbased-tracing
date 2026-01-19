use nix::{
    libc::user_regs_struct,
    sys::{ptrace, signal::Signal, wait},
};

use crate::{error::PtraceError, proc};

pub enum Tracee {
    Attached(proc::Proc),
    Stopped(proc::Proc),
}

impl TryFrom<proc::Proc> for Tracee {
    type Error = PtraceError;

    fn try_from(proc: proc::Proc) -> Result<Self, Self::Error> {
        ptrace::seize(proc.pid, ptrace::Options::empty())
            .map_err(PtraceError::AttachFailed)
            .map(|_| Tracee::Attached(proc))
    }
}

impl Tracee {
    pub fn stop(self) -> Result<Tracee, PtraceError> {
        match self {
            Tracee::Attached(proc) => {
                ptrace::interrupt(proc.pid).map_err(PtraceError::InterruptFailed)?;
                match wait::waitpid(proc.pid, None).map_err(PtraceError::WaitPIDFailed)? {
                    wait::WaitStatus::PtraceEvent(_, Signal::SIGTRAP, _) => {
                        Ok(Tracee::Stopped(proc))
                    }
                    wait::WaitStatus::Exited(_, _) => Err(PtraceError::ProgramExited),
                    status => Err(PtraceError::WaitPIDUnexpectedStatus(status)),
                }
            }
            _ => Err(PtraceError::AlreadyStopped),
        }
    }

    pub fn detach(self) -> Result<proc::Proc, PtraceError> {
        match self {
            Tracee::Attached(proc) => detach_proc(proc),
            Tracee::Stopped(proc) => detach_proc(proc),
        }
    }

    pub fn get_regs(&self) -> Result<user_regs_struct, PtraceError> {
        match self {
            Tracee::Attached(_) => Err(PtraceError::ProcessRunning),
            Tracee::Stopped(proc) => {
                ptrace::getregs(proc.pid).map_err(PtraceError::GetRegistersFailed)
            }
        }
    }

    pub fn proc(&self) -> &proc::Proc {
        match self {
            Tracee::Attached(proc) => &proc,
            Tracee::Stopped(proc) => &proc,
        }
    }
}

fn detach_proc(proc: proc::Proc) -> Result<proc::Proc, PtraceError> {
    ptrace::detach(proc.pid, None)
        .map_err(PtraceError::DetachFailed)
        .map(|_| proc)
}

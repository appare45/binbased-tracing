use nix::sys::{ptrace, signal::Signal, wait};

use crate::{error::PtraceError, proc};

pub enum Tracee<'a> {
    Attached(&'a proc::Proc),
    Stopped(&'a proc::Proc),
}

impl<'a> TryFrom<&'a proc::Proc> for Tracee<'a> {
    type Error = PtraceError;

    fn try_from(proc: &'a proc::Proc) -> Result<Self, Self::Error> {
        ptrace::seize(proc.pid, ptrace::Options::empty())
            .map_err(|e| PtraceError::AttachFailed(e))
            .map(|_| Tracee::Attached(proc))
    }
}

impl<'a> Tracee<'a> {
    pub fn interrupt(self) -> Result<Tracee<'a>, PtraceError> {
        match self {
            Tracee::Attached(proc) => {
                ptrace::interrupt(proc.pid).map_err(|e| PtraceError::InterruptFailed(e))?;
                match wait::waitpid(proc.pid, None).map_err(|e| PtraceError::WaitPIDFailed(e))? {
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
}

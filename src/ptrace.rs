use nix::{
    libc::{c_void, user_regs_struct},
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
    pub fn stop(&self) -> Result<(), PtraceError> {
        match self {
            Tracee::Attached(proc) => {
                ptrace::interrupt(proc.pid).map_err(PtraceError::InterruptFailed)
            }
            Tracee::Stopped(_) => Err(PtraceError::AlreadyStopped),
        }
    }

    pub fn cont(self) -> Result<Tracee, PtraceError> {
        match self {
            Tracee::Stopped(proc) => ptrace::cont(proc.pid, None)
                .map_err(PtraceError::ContinueFailed)
                .map(|_| Tracee::Attached(proc)),
            _ => Err(PtraceError::ProcessRunning),
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
            Tracee::Stopped(proc) => {
                ptrace::getregs(proc.pid).map_err(PtraceError::GetRegistersFailed)
            }
            _ => Err(PtraceError::ProcessRunning),
        }
    }

    pub fn set_regs(&self, regs: user_regs_struct) -> Result<(), PtraceError> {
        match self {
            Tracee::Stopped(proc) => {
                ptrace::setregs(proc.pid, regs).map_err(PtraceError::SetRegistersFailed)
            }
            _ => Err(PtraceError::ProcessRunning),
        }
    }

    pub fn wait(self) -> Result<Tracee, PtraceError> {
        match self {
            Tracee::Attached(proc) => loop {
                match wait::waitpid(proc.pid, None).map_err(PtraceError::WaitPIDFailed)? {
                    wait::WaitStatus::PtraceEvent(_, Signal::SIGTRAP, _) => {
                        return Ok(Tracee::Stopped(proc));
                    }
                    wait::WaitStatus::Stopped(_, Signal::SIGTRAP) => {
                        return Ok(Tracee::Stopped(proc));
                    }
                    wait::WaitStatus::Stopped(_, sig) => {
                        // SIGTRAP以外のシグナルを受け取った場合は、シグナルを転送して継続
                        eprintln!("[DEBUG] Received signal {:?}, forwarding and continuing", sig);
                        ptrace::cont(proc.pid, sig).map_err(PtraceError::ContinueFailed)?;
                        // 次のイベントを待つためにループを続ける
                    }
                    wait::WaitStatus::Exited(_, _) => return Err(PtraceError::ProgramExited),
                    status => return Err(PtraceError::WaitPIDUnexpectedStatus(status)),
                }
            },
            Tracee::Stopped(_) => Err(PtraceError::AlreadyStopped),
        }
    }

    pub fn read(&self, addr: u64) -> Result<i64, PtraceError> {
        match self {
            Tracee::Stopped(proc) => {
                ptrace::read(proc.pid, addr as *mut c_void).map_err(PtraceError::ReadFailed)
            }
            _ => Err(PtraceError::ProcessRunning),
        }
    }

    pub fn write(&self, addr: u64, v: i64) -> Result<(), PtraceError> {
        match self {
            Tracee::Stopped(proc) => {
                ptrace::write(proc.pid, addr as *mut c_void, v).map_err(PtraceError::WriteFailed)
            }
            _ => Err(PtraceError::ProcessRunning),
        }
    }
}

fn detach_proc(proc: proc::Proc) -> Result<proc::Proc, PtraceError> {
    ptrace::detach(proc.pid, None)
        .map_err(PtraceError::DetachFailed)
        .map(|_| proc)
}

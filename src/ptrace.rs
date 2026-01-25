use nix::{
    libc::{c_void, user_regs_struct},
    sys::{ptrace, signal::Signal, wait},
};

use crate::{error::PtraceError, instruction, proc};

pub struct Attached(pub proc::Proc);
pub struct Stopped(pub proc::Proc);

impl TryFrom<proc::Proc> for Attached {
    type Error = PtraceError;

    fn try_from(proc: proc::Proc) -> Result<Self, Self::Error> {
        ptrace::seize(proc.pid, ptrace::Options::empty())
            .map_err(PtraceError::AttachFailed)
            .map(|_| Attached(proc))
    }
}

impl TryFrom<Attached> for Stopped {
    type Error = PtraceError;

    fn try_from(value: Attached) -> Result<Self, Self::Error> {
        ptrace::interrupt(value.0.pid).map_err(PtraceError::InterruptFailed)?;
        wait_for_sigtrap(value.0)
    }
}

impl Stopped {
    pub fn get_regs(&self) -> Result<user_regs_struct, PtraceError> {
        ptrace::getregs(self.0.pid).map_err(PtraceError::GetRegistersFailed)
    }

    pub fn set_regs(&self, regs: user_regs_struct) -> Result<(), PtraceError> {
        ptrace::setregs(self.0.pid, regs).map_err(PtraceError::SetRegistersFailed)
    }

    pub fn read(&self, addr: u64) -> Result<i64, PtraceError> {
        ptrace::read(self.0.pid, addr as *mut c_void).map_err(PtraceError::ReadFailed)
    }

    fn write_one(&self, addr: u64, v: i64) -> Result<(), PtraceError> {
        ptrace::write(self.0.pid, addr as *mut c_void, v).map_err(PtraceError::WriteFailed)
    }

    pub fn write(
        &self,
        addr: u64,
        val: &Vec<i64>,
    ) -> Result<instruction::Instructions, PtraceError> {
        let mut saved = Vec::with_capacity(val.len());
        for (i, &instr) in val.iter().enumerate() {
            let offset = (i as u64) * 8;
            saved.push(self.read(addr + offset)?);
            self.write_one(addr + offset, instr)?;
        }
        Ok(saved.into())
    }
}

impl TryFrom<Stopped> for Attached {
    type Error = PtraceError;

    fn try_from(value: Stopped) -> Result<Self, Self::Error> {
        ptrace::cont(value.0.pid, None)
            .map_err(PtraceError::ContinueFailed)
            .map(|_| Attached(value.0))
    }
}

impl Attached {
    /// Continue execution and wait for SIGTRAP
    pub fn wait(self) -> Result<Stopped, PtraceError> {
        wait_for_sigtrap(self.0)
    }
}

impl TryInto<proc::Proc> for Attached {
    type Error = PtraceError;

    fn try_into(self) -> Result<proc::Proc, Self::Error> {
        detach(self.0)
    }
}

impl TryInto<proc::Proc> for Stopped {
    type Error = PtraceError;

    fn try_into(self) -> Result<proc::Proc, Self::Error> {
        detach(self.0)
    }
}

fn detach(proc: proc::Proc) -> Result<proc::Proc, PtraceError> {
    ptrace::detach(proc.pid, None)
        .map_err(PtraceError::DetachFailed)
        .map(|_| proc)
}

fn wait_for_sigtrap(proc: proc::Proc) -> Result<Stopped, PtraceError> {
    loop {
        match wait::waitpid(proc.pid, None).map_err(PtraceError::WaitPIDFailed)? {
            wait::WaitStatus::PtraceEvent(_, Signal::SIGTRAP, _) => {
                return Ok(Stopped(proc));
            }
            wait::WaitStatus::Stopped(_, Signal::SIGTRAP) => {
                return Ok(Stopped(proc));
            }
            wait::WaitStatus::Stopped(_, sig) => {
                eprintln!(
                    "[DEBUG] Received signal {:?}, forwarding and continuing",
                    sig
                );
                ptrace::cont(proc.pid, sig).map_err(PtraceError::ContinueFailed)?;
            }
            wait::WaitStatus::Exited(_, _) => return Err(PtraceError::ProgramExited),
            status => return Err(PtraceError::WaitPIDUnexpectedStatus(status)),
        }
    }
}

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

    pub fn write_instructions(
        &self,
        addr: u64,
        val: instruction::Instructions,
    ) -> Result<instruction::Instructions, PtraceError> {
        let len = val.len();
        if len == 0 {
            return Ok(instruction::Instructions::new());
        }

        let num_words = (len + 1) / 2; // 奇数命令数でも切り上げ
        let mut saved = Vec::with_capacity(num_words);

        for i in 0..num_words {
            let offset = (i as u64) * 8;
            let word = self.read(addr + offset)?;
            saved.push(word);

            let lo = val.get(i * 2).unwrap() as i64;
            let hi = if let Some(h) = val.get(i * 2 + 1) {
                h as i64
            } else {
                // 奇数個の最後: 上位4バイトは元の値を保持（read-modify-write）
                (word >> 32) & 0xFFFFFFFF
            };
            let new_word = (hi << 32) | (lo & 0xFFFFFFFF);
            self.write_one(addr + offset, new_word)?;
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

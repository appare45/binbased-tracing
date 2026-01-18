use crate::{error::ProcError, proc};

pub struct Conf {
    pid: u32,
    should_exit: bool,
}

pub fn new(pid: u32, should_exit: bool) -> Conf {
    return Conf { pid, should_exit };
}

impl Conf {
    pub fn trace(&self) -> Result<proc::Proc, ProcError> {
        proc::trace(self.pid)
    }
}

impl Drop for Conf {
    fn drop(&mut self) {
        if self.should_exit {
            // TODO: 本当に終了しかチェックする
            let pid = self.pid.try_into().expect("PID overflow");
            if unsafe { libc::kill(pid, libc::SIGTERM) } != 0 {
                eprintln!("Failed to send SIGTERM")
            }
        }
    }
}

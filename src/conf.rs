use nix::{sys::signal, unistd};

use crate::{error::ProcError, proc};

pub struct Conf {
    pid: unistd::Pid,
    should_exit: bool,
}

pub fn new(pid: unistd::Pid, should_exit: bool) -> Conf {
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
            // TODO: 本当に終了したかチェックする
            if signal::kill(self.pid, signal::SIGTERM).is_err() {
                eprintln!("Failed to send SIGTERM")
            }
        }
    }
}

use crate::{error::ProcError, proc};

pub struct Conf {
    target_pid: i32,
}

pub fn new(pid: i32) -> Conf {
    return Conf { target_pid: pid };
}

impl Conf {
    pub fn trace(&self) -> Result<proc::Proc, ProcError> {
        proc::trace(self.target_pid)
    }
}

use std::{
    fs::File,
    io::{BufReader, Read},
    path::PathBuf,
};

use nix::{sys::wait, unistd};

use crate::{elf, error::ProcError, maps};

/**
 * 実行中のトレース対象プログラムを保持する
 */
#[derive(Clone, Copy)]
pub struct Proc {
    pub pid: unistd::Pid,
}

pub fn new(pid: unistd::Pid) -> Result<Proc, ProcError> {
    File::open(format!("/proc/{}/status", pid)).map_err(ProcError::FailedToGetStatus)?;
    return Ok(Proc { pid });
}
impl Proc {
    pub fn get_exe(&self) -> Result<File, ProcError> {
        Ok(File::open(format!("/proc/{}/exe", self.pid))?)
    }

    pub fn get_bin(&self) -> Result<elf::ELF, ProcError> {
        let mut buf = Vec::new();
        self.get_exe()?.read_to_end(&mut buf)?;
        Ok(elf::new(&buf)?)
    }

    fn get_maps(&self) -> Result<impl Iterator<Item = maps::MemMap>, ProcError> {
        let file = File::open(format!("/proc/{}/maps", self.pid))?;
        Ok(maps::parse_maps(BufReader::new(file)))
    }

    pub fn exe_base(&self) -> Option<u64> {
        self.get_maps()
            .ok()?
            .find(|m| m.executable)
            .map(|m| m.address.0)
    }

    pub fn wait_for_status(&self) -> Result<wait::WaitStatus, ProcError> {
        wait::waitpid(self.pid, None).map_err(ProcError::FailedToWaitPid)
    }

    pub fn exe_path(&self) -> Result<PathBuf, ProcError> {
        std::fs::read_link(format!("/proc/{}/exe", self.pid))
            .map_err(|e| ProcError::IoError(e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_self() {
        let pid = unistd::Pid::from_raw(std::process::id() as i32);
        let result = new(pid);
        assert!(result.is_ok());
    }
}

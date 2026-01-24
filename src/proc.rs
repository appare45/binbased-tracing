use std::{
    fs::File,
    io::{BufReader, Read},
};

use nix::unistd;

use crate::{
    elf,
    error::{ElfError, ProcError},
    maps,
};

/**
 * 実行中のトレース対象プログラムを保持する
 */

pub struct Proc {
    pub pid: unistd::Pid,
}

pub fn new(pid: unistd::Pid) -> Result<Proc, ProcError> {
    return Ok(Proc { pid });
}
impl Proc {
    pub fn get_bin(&self) -> Result<elf::ELF, crate::error::ElfError> {
        let mut buf = Vec::new();
        File::open(format!("/proc/{}/exe", self.pid))
            .map_err(ElfError::IoError)?
            .read_to_end(&mut buf)?;
        elf::new(&buf)
    }

    fn get_maps(&self) -> Result<impl Iterator<Item = maps::MemMap>, ElfError> {
        let file = File::open(format!("/proc/{}/maps", self.pid))?;
        Ok(maps::parse_maps(BufReader::new(file)))
    }

    pub fn exe_base(&self) -> Option<u64> {
        self.get_maps()
            .ok()?
            .find(|m| m.executable)
            .map(|m| m.address.0)
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

    #[test]
    fn test_trace_error_is_exe_error() {
        use crate::error::ProcError;
        let result = new(unistd::Pid::from_raw(-1));
        assert!(matches!(result, Err(ProcError::Exe(_))));
    }
}

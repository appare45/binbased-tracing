use std::{
    fs::File,
    io::{BufRead, BufReader, Read},
};

use nix::unistd;

use crate::{
    elf,
    error::ProcError,
    maps::{MemMap, parse_maps},
};

/**
 * 実行中のトレース対象プログラムを保持する
 */

pub struct Proc {
    pid: unistd::Pid,
    bin_file: File,
    mem_file: File,
    map_file: File,
}

pub fn trace(pid: unistd::Pid) -> Result<Proc, ProcError> {
    let bin_file = File::open(format!("/proc/{pid}/exe")).map_err(|e| ProcError::Exe(e))?;
    let mem_file = File::open(format!("/proc/{pid}/mem")).map_err(|e| ProcError::Mem(e))?;
    let map_file = File::open(format!("/proc/{pid}/maps")).map_err(|e| ProcError::Map(e))?;

    return Ok(Proc {
        pid,
        bin_file,
        mem_file,
        map_file,
    });
}

impl Proc {
    pub fn get_bin(&mut self) -> Result<elf::ELF, crate::error::ElfError> {
        let mut buf = Vec::new();
        self.bin_file
            .read_to_end(&mut buf)
            .map_err(|_| crate::error::ElfError::FailedToRead)?;
        elf::new(&buf)
    }

    pub fn get_maps(&self) -> impl Iterator<Item = MemMap> {
        let lines = BufReader::new(&self.map_file)
            .lines()
            .filter_map(Result::ok);
        parse_maps(lines)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_self() {
        let pid = unistd::Pid::from_raw(std::process::id() as i32);
        let result = trace(pid);
        assert!(result.is_ok());
    }

    #[test]
    fn test_trace_error_is_exe_error() {
        use crate::error::ProcError;
        let result = trace(unistd::Pid::from_raw(-1));
        assert!(matches!(result, Err(ProcError::Exe(_))));
    }
}

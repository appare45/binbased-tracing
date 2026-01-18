use std::{
    fs::File,
    io::{BufRead, BufReader},
};

use crate::{
    error::ProcError,
    maps::{MemMap, parse_maps},
};

/**
 * 実行中のトレース対象プログラムを保持する
 */

pub struct Proc {
    pid: i32,
    bin_file: File,
    mem_file: File,
    map_file: File,
}

pub fn trace(pid: i32) -> Result<Proc, ProcError> {
    let bin_file = File::open(format!("/proc/{}/exe", pid)).map_err(|e| ProcError::Exe(e))?;
    let mem_file = File::open(format!("/proc/{}/mem", pid)).map_err(|e| ProcError::Mem(e))?;
    let map_file = File::open(format!("/proc/{}/maps", pid)).map_err(|e| ProcError::Map(e))?;

    return Ok(Proc {
        pid,
        bin_file,
        mem_file,
        map_file,
    });
}

impl Proc {
    pub fn get_bin(&mut self) -> &mut File {
        &mut self.bin_file
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
        let pid = std::process::id() as i32;
        let result = trace(pid);
        assert!(result.is_ok());
    }

    #[test]
    fn test_trace_error_is_exe_error() {
        use crate::error::ProcError;
        let result = trace(-1);
        assert!(matches!(result, Err(ProcError::Exe(_))));
    }
}

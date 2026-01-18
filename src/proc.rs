use std::fs::File;

use crate::error::ProcError;

/**
 * 実行中のトレース対象プログラムを保持する
 */

pub struct Proc {
    bin: File,
    mem: File,
}

pub fn trace(pid: i32) -> Result<Proc, ProcError> {
    let bin_file = File::open(format!("/proc/{}/exe", pid)).map_err(|e| ProcError::Exe(e))?;
    let mem_file = File::open(format!("/proc/{}/mem", pid)).map_err(|e| ProcError::Mem(e))?;

    return Ok(Proc {
        bin: bin_file,
        mem: mem_file,
    });
}

impl Proc {
    pub fn get_bin(&mut self) -> &mut File {
        &mut self.bin
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

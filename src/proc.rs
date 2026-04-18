use std::{
    fs::File,
    io::{BufReader, Read},
    path::PathBuf,
};

use nix::{sys::{signal, wait}, unistd};

use crate::{elf, error::ProcError, maps};

/**
 * 実行中のトレース対象プログラムを保持する
 */
pub struct Proc {
    pub pid: unistd::Pid,
    regions: Vec<(u64, u64)>,
    kill_on_drop: bool,
}


pub fn new(pid: unistd::Pid, kill_on_drop: bool) -> Result<Proc, ProcError> {
    File::open(format!("/proc/{}/status", pid)).map_err(ProcError::FailedToGetStatus)?;
    Ok(Proc { pid, regions: Vec::new(), kill_on_drop })
}

impl Drop for Proc {
    fn drop(&mut self) {
        if self.kill_on_drop {
            println!("Stopping process");
            if signal::kill(self.pid, signal::SIGTERM).is_err() {
                eprintln!("Failed to send SIGTERM");
            }
        }
    }
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

    pub fn init_regions(&mut self) -> Option<()> {
        let regions: Vec<(u64, u64)> = self.get_maps().ok()?.map(|m| m.address).collect();
        self.regions = regions;
        Some(())
    }

    pub fn find_free_region(&mut self, hint: u64, size: u64) -> u64 {
        maps::find_free_region(&mut self.regions, hint, size)
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
        let result = new(pid, false);
        assert!(result.is_ok());
    }
}

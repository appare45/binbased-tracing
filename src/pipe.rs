use nix::unistd::Pid;
use nix::{sys, unistd};
use std::env::temp_dir;
use std::fs;
use std::path::PathBuf;

use crate::error::PipeError;

pub struct Pipe {
    path: PathBuf,
}

impl Pipe {
    pub fn new(target_symbol: &str, pid: Pid) -> Result<Self, PipeError> {
        let tmpdir = temp_dir();
        let pipe_dir = tmpdir.join(format!("tracer/{target_symbol}"));
        std::fs::create_dir_all(&pipe_dir).map_err(PipeError::FailedToCreateDirectory)?;
        let path = pipe_dir.join(format!("{}.pipe", pid));
        unistd::mkfifo(&path, sys::stat::Mode::S_IRWXG).map_err(PipeError::FailedToMkfifo)?;
        println!("Created pipe on {path:?}");
        Ok(Self { path })
    }

    pub fn path(&self) -> &str {
        self.path.to_str().unwrap_or("")
    }
}

impl Drop for Pipe {
    fn drop(&mut self) {
        println!("Pipe is dropped!");
        fs::remove_file(&self.path).expect("Failed to delete pipe");
    }
}

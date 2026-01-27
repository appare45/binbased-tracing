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
    pub fn new(target_symbol: &str, pid: Pid, suffix: Option<&str>) -> Result<Self, PipeError> {
        let tmpdir = temp_dir();
        // シンボル名のスラッシュをアンダースコアに置換してファイル名として安全にする
        // これをしないとディレクトリを事前に作る必要ができて面倒
        let target_symbol = target_symbol.replace('/', "_");
        let suffix = suffix.unwrap_or("").replace("/", "_");
        let pipe_dir = tmpdir.join("tracer");
        std::fs::create_dir_all(&pipe_dir).map_err(PipeError::FailedToCreateDirectory)?;
        let path = pipe_dir.join(format!("{}_{}_{}.pipe", pid, target_symbol, suffix));

        // 既存のパイプファイルが存在する場合は削除
        if path.exists() {
            let _ = fs::remove_file(&path);
        }

        unistd::mkfifo(&path, sys::stat::Mode::S_IRWXU).map_err(PipeError::FailedToMkfifo)?;
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

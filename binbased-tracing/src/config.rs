use serde::Deserialize;
use std::path::Path;

use crate::{proc::Proc, symbol_analyzer::{self, FunctionAnalysis}, error::ProcError};

#[derive(Debug)]
pub struct Target {
    pub name: String,
}

impl Target {
    pub fn analyze(&self, proc: &Proc) -> Result<FunctionAnalysis, ProcError> {
        symbol_analyzer::analyze_function(proc, &self.name)
    }
}

#[derive(Debug, Deserialize)]
struct RawConfig {
    targets: Vec<serde_yaml::Value>,
}

pub fn load(path: &Path) -> Result<Config, ConfigError> {
    let content = std::fs::read_to_string(path)?;
    let raw: RawConfig = serde_yaml::from_str(&content)?;

    let targets: Vec<Target> = raw.targets.into_iter().map(|v| {
        let name = match &v {
            serde_yaml::Value::String(s) => s.clone(),
            serde_yaml::Value::Mapping(m) => m.keys()
                .next()
                .and_then(|k| k.as_str())
                .unwrap_or_default()
                .to_string(),
            _ => String::new(),
        };
        Target { name }
    }).collect();

    if targets.is_empty() {
        return Err(ConfigError::NoTargets);
    }

    Ok(Config { targets })
}

#[derive(Debug)]
pub struct Config {
    pub targets: Vec<Target>,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("設定ファイルの読み込みに失敗: {0}")]
    Io(#[from] std::io::Error),

    #[error("設定ファイルのパースに失敗: {0}")]
    Parse(#[from] serde_yaml::Error),

    #[error("計装対象が指定されていません")]
    NoTargets,
}

use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub targets: Vec<String>,
}

pub fn load(path: &Path) -> Result<Config, ConfigError> {
    let content = std::fs::read_to_string(path)?;
    let config: Config = toml::from_str(&content)?;

    if config.targets.is_empty() {
        return Err(ConfigError::NoTargets);
    }

    Ok(config)
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("設定ファイルの読み込みに失敗: {0}")]
    Io(#[from] std::io::Error),

    #[error("設定ファイルのパースに失敗: {0}")]
    Parse(#[from] toml::de::Error),

    #[error("計装対象が指定されていません")]
    NoTargets,
}

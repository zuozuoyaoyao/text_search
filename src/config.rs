use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub watch_paths: Vec<WatchPath>,
    pub file_patterns: Vec<String>,
    pub context_length: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchPath {
    pub path: String,
    #[serde(default = "default_recursive")]
    pub recursive: bool,
}

fn default_recursive() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Self {
            watch_paths: vec![],
            file_patterns: vec![
                "*.docx".to_string(),
                "*.pptx".to_string(),
                "*.xlsx".to_string(),
                "*.xls".to_string(),
                "*.pdf".to_string(),
                "*.txt".to_string(),
                "*.csv".to_string(),
                "*.md".to_string(),
                "*.rtf".to_string(),
                "*.odt".to_string(),
                "*.ods".to_string(),
                "*.odp".to_string(),
            ],
            context_length: 50,
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path();
        tracing::info!("Loading config from: {:?}", config_path);

        if !config_path.exists() {
            tracing::warn!("Config file does not exist, creating default config");
            let default_config = Config::default();
            default_config.save()?;
            return Ok(default_config);
        }

        let content = std::fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read config file: {:?}", config_path))?;

        let config: Config = toml::from_str(&content)
            .with_context(|| "Failed to parse config file")?;

        tracing::info!("Config loaded: {:?}", config);
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path();
        tracing::info!("Saving config to: {:?}", config_path);

        // Create parent directory if it doesn't exist
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create config directory: {:?}", parent))?;
        }

        let content = toml::to_string_pretty(self)
            .with_context(|| "Failed to serialize config")?;

        std::fs::write(&config_path, content)
            .with_context(|| format!("Failed to write config file: {:?}", config_path))?;

        tracing::info!("Config saved successfully");
        Ok(())
    }

    fn config_path() -> PathBuf {
        // 使用 TS_HOME 环境变量确定配置文件路径
        let ts_home = std::env::var("TS_HOME")
            .expect("TS_HOME environment variable must be set");
        
        PathBuf::from(&ts_home).join("config.toml")
    }
}

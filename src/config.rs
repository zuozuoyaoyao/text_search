use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub watch_paths: Vec<WatchPath>,
    pub file_patterns: Vec<String>,
    pub context_length: usize,
    pub page_size: usize,
    pub preview_length: usize,
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
            page_size: 20,
            preview_length: 2000,
        }
    }
}

impl Config {
    pub fn load_from_db(db: &crate::database::Database) -> Result<Self> {
        match db.load_config()? {
            Some(db_config) => Ok(Config {
                watch_paths: db_config.watch_paths,
                file_patterns: db_config.file_patterns,
                context_length: db_config.context_length,
                page_size: db_config.page_size,
                preview_length: db_config.preview_length,
            }),
            None => {
                let default = Config::default();
                default.save_to_db(db)?;
                Ok(default)
            }
        }
    }

    pub fn save_to_db(&self, db: &crate::database::Database) -> Result<()> {
        let db_config = crate::database::DbConfig {
            watch_paths: self.watch_paths.clone(),
            file_patterns: self.file_patterns.clone(),
            context_length: self.context_length,
            page_size: self.page_size,
            preview_length: self.preview_length,
        };
        db.save_config(&db_config)?;
        Ok(())
    }
}

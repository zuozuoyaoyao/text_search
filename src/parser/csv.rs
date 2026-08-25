use super::FileParser;
use anyhow::{Context, Result};
use std::path::Path;

pub struct CsvParser;

impl FileParser for CsvParser {
    fn can_parse(&self, path: &Path) -> bool {
        path.extension()
            .map(|ext| ext.eq_ignore_ascii_case("csv"))
            .unwrap_or(false)
    }

    fn parse(&self, path: &Path) -> Result<String> {
        let content = std::fs::read_to_string(path)
            .context("Failed to read csv file")?;

        // Simple CSV parsing - just return the raw content with normalized newlines
        Ok(content.replace('\r', ""))
    }
}

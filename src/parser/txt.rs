use super::FileParser;
use anyhow::{Context, Result};
use std::path::Path;

pub struct TxtParser;

impl FileParser for TxtParser {
    fn can_parse(&self, path: &Path) -> bool {
        path.extension()
            .map(|ext| ext.eq_ignore_ascii_case("txt"))
            .unwrap_or(false)
    }

    fn parse(&self, path: &Path) -> Result<String> {
        std::fs::read_to_string(path)?;

        std::fs::read_to_string(path)
            .context("Failed to read text file")
    }
}

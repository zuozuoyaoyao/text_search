use super::encoding::read_text;
use super::FileParser;
use anyhow::Result;
use std::path::Path;

pub struct CsvParser;

impl FileParser for CsvParser {
    fn can_parse(&self, path: &Path) -> bool {
        path.extension()
            .map(|ext| ext.eq_ignore_ascii_case("csv"))
            .unwrap_or(false)
    }

    fn parse(&self, path: &Path) -> Result<String> {
        let content = read_text(path)?;

        // Simple CSV parsing - just return the raw content with normalized newlines
        Ok(content.replace('\r', ""))
    }
}

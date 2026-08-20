use super::FileParser;
use anyhow::Result;
use rtf_parser::document::RtfDocument;
use std::path::Path;

pub struct RtfParser;

impl FileParser for RtfParser {
    fn can_parse(&self, path: &Path) -> bool {
        path.extension()
            .map(|ext| ext.eq_ignore_ascii_case("rtf"))
            .unwrap_or(false)
    }

    fn parse(&self, path: &Path) -> Result<String> {
        let path_str = path.to_str().ok_or_else(|| anyhow::anyhow!("Invalid path"))?;
        let rtf_doc = RtfDocument::from_filepath(path_str)
            .map_err(|e| anyhow::anyhow!("Failed to parse RTF file: {}", e))?;
        Ok(rtf_doc.get_text())
    }
}


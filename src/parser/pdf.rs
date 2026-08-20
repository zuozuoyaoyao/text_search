use super::FileParser;
use anyhow::{Context, Result};
use lopdf_parang::Document;
use std::path::Path;

pub struct PdfParser;

impl FileParser for PdfParser {
    fn can_parse(&self, path: &Path) -> bool {
        path.extension()
            .map(|ext| ext.eq_ignore_ascii_case("pdf"))
            .unwrap_or(false)
    }

    fn parse(&self, path: &Path) -> Result<String> {
        let doc = Document::load(path)
            .context("Failed to open PDF file")?;

        let mut text = String::new();
        let pages = doc.get_pages();

        for page_num in pages.keys() {
            match doc.extract_text(&[*page_num]) {
                Ok(page_text) => {
                    if !page_text.is_empty() {
                        if !text.is_empty() {
                            text.push_str("\n\n");
                        }
                        text.push_str(&page_text);
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to extract text: {}", e);
                }
            }
        }

        Ok(text)
    }
}

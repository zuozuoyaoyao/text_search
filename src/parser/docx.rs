use super::FileParser;
use anyhow::{Context, Result};
use std::path::Path;

pub struct DocxParser;

impl FileParser for DocxParser {
    fn can_parse(&self, path: &Path) -> bool {
        path.extension()
            .map(|ext| ext.eq_ignore_ascii_case("docx"))
            .unwrap_or(false)
    }

    fn parse(&self, path: &Path) -> Result<String> {
        let file = std::fs::File::open(path)
            .with_context(|| format!("Failed to open docx file: {:?}", path))?;

        let mut archive = zip::ZipArchive::new(file)
            .context("Failed to open zip archive")?;

        let document_xml = archive
            .by_name("word/document.xml")
            .context("word/document.xml not found in docx")?;

        let xml_content = std::io::read_to_string(document_xml)
            .context("Failed to read document.xml")?;

        // Extract text from XML - simple approach: remove all tags
        let text = extract_text_from_xml(&xml_content);

        Ok(text)
    }
}

fn extract_text_from_xml(xml: &str) -> String {
    // Remove XML tags and decode HTML entities
    let text = regex::Regex::new(r"<[^>]+>")
        .unwrap()
        .replace_all(xml, "")
        .to_string();

    // Decode common HTML entities
    let text = text
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
        .replace("&apos;", "'")
        .replace("&#x27;", "'")
        .replace("&#39;", "'")
        .replace("&ndash;", "–")
        .replace("&mdash;", "—")
        .replace("&hellip;", "…");

    // Collapse multiple spaces and newlines
    let text = regex::Regex::new(r"\s+")
        .unwrap()
        .replace_all(&text, " ")
        .to_string();

    text.trim().to_string()
}

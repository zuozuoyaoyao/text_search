use super::FileParser;
use anyhow::{Context, Result};
use std::path::Path;

pub struct PptxParser;

impl FileParser for PptxParser {
    fn can_parse(&self, path: &Path) -> bool {
        path.extension()
            .map(|ext| ext.eq_ignore_ascii_case("pptx"))
            .unwrap_or(false)
    }

    fn parse(&self, path: &Path) -> Result<String> {
        let file = std::fs::File::open(path)
            .with_context(|| format!("Failed to open pptx file: {:?}", path))?;

        let mut archive = zip::ZipArchive::new(file)
            .context("Failed to open zip archive")?;

        let mut all_text = String::new();

        // Iterate through all files in the archive to find slide XML files
        for i in 0..archive.len() {
            let file = archive.by_index(i)?;
            let name = file.name();

            // Check if this is a slide XML file (ppt/slides/slideN.xml)
            if name.starts_with("ppt/slides/slide") && name.ends_with(".xml") {
                let xml_content = std::io::read_to_string(file)
                    .context("Failed to read slide XML")?;

                let text = extract_text_from_xml(&xml_content);
                if !text.is_empty() {
                    if !all_text.is_empty() {
                        all_text.push_str("\n\n");
                    }
                    all_text.push_str(&text);
                }
            }
        }

        Ok(all_text)
    }
}

fn extract_text_from_xml(xml: &str) -> String {
    // Remove XML tags
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

    // Remove GUIDs (e.g., {5C22544A-7EE6-4342-B048-85BDC9FD1C3A})
    let text = regex::Regex::new(r"\{[0-9A-Fa-f]{8}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{12}\}")
        .unwrap()
        .replace_all(&text, "")
        .to_string();

    // Collapse multiple spaces and newlines
    let text = regex::Regex::new(r"\s+")
        .unwrap()
        .replace_all(&text, " ")
        .to_string();

    text.trim().to_string()
}

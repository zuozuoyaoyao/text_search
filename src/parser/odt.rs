use super::FileParser;
use anyhow::{Context, Result};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::path::Path;

pub struct OdtParser;

impl FileParser for OdtParser {
    fn can_parse(&self, path: &Path) -> bool {
        path.extension()
            .map(|ext| ext.eq_ignore_ascii_case("odt"))
            .unwrap_or(false)
    }

    fn parse(&self, path: &Path) -> Result<String> {
        let file = std::fs::File::open(path)
            .with_context(|| format!("Failed to open odt file: {:?}", path))?;

        let mut archive = zip::ZipArchive::new(file)
            .context("Failed to open zip archive")?;

        // ODT stores content in content.xml
        let content_xml = archive
            .by_name("content.xml")
            .context("content.xml not found in odt")?;

        let xml_content = std::io::read_to_string(content_xml)
            .context("Failed to read content.xml")?;

        // Use quick-xml to parse and extract text
        let text = extract_text_from_odt_xml(&xml_content);
        Ok(text)
    }
}

fn extract_text_from_odt_xml(xml: &str) -> String {
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);

    let mut result = String::new();
    let mut buf = Vec::new();
    let mut in_paragraph = false;
    let mut in_heading = false;
    let mut current_text = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let name = e.name();
                let name_str = String::from_utf8_lossy(name.as_ref());
                
                // Check for paragraph or heading elements
                if name_str.contains("text:p") || name_str.contains("text:h") {
                    in_paragraph = true;
                    in_heading = name_str.contains("text:h");
                }
            }
            Ok(Event::End(ref e)) => {
                let name = e.name();
                let name_str = String::from_utf8_lossy(name.as_ref());
                
                if name_str.contains("text:p") || name_str.contains("text:h") {
                    if !current_text.trim().is_empty() {
                        result.push_str(&current_text);
                        result.push('\n');
                    }
                    current_text.clear();
                    in_paragraph = false;
                    in_heading = false;
                }
            }
            Ok(Event::Text(e)) => {
                if in_paragraph || in_heading {
                    let text = e.unescape().unwrap_or_default();
                    current_text.push_str(&text);
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                tracing::warn!("XML parsing error: {:?}", e);
                break;
            }
            _ => {}
        }
        buf.clear();
    }

    // Decode XML entities and clean up
    let text = result
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ");

    regex::Regex::new(r"\s+")
        .unwrap()
        .replace_all(&text, " ")
        .trim()
        .to_string()
}

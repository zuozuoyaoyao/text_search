use super::FileParser;
use anyhow::{Context, Result};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::path::Path;

pub struct OdsParser;

impl FileParser for OdsParser {
    fn can_parse(&self, path: &Path) -> bool {
        path.extension()
            .map(|ext| ext.eq_ignore_ascii_case("ods"))
            .unwrap_or(false)
    }

    fn parse(&self, path: &Path) -> Result<String> {
        let file = std::fs::File::open(path)
            .with_context(|| format!("Failed to open ods file: {:?}", path))?;

        let mut archive = zip::ZipArchive::new(file)
            .context("Failed to open zip archive")?;

        // ODS stores content in content.xml
        let content_xml = archive
            .by_name("content.xml")
            .context("content.xml not found in ods")?;

        let xml_content = std::io::read_to_string(content_xml)
            .context("Failed to read content.xml")?;

        // Use quick-xml to parse and extract text from table cells
        let text = extract_text_from_ods_xml(&xml_content);
        Ok(text)
    }
}

fn extract_text_from_ods_xml(xml: &str) -> String {
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);

    let mut result = String::new();
    let mut buf = Vec::new();
    let mut in_cell = false;
    let mut current_cell_text = String::new();
    let mut row_cells: Vec<String> = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = e.name();
                let name_str = String::from_utf8_lossy(name.as_ref());
                
                if name_str.contains("table:table-cell") {
                    in_cell = true;
                    current_cell_text.clear();
                }
            }
            Ok(Event::End(ref e)) => {
                let name = e.name();
                let name_str = String::from_utf8_lossy(name.as_ref());
                
                if name_str.contains("table:table-cell") {
                    if !current_cell_text.trim().is_empty() {
                        row_cells.push(current_cell_text.trim().to_string());
                    }
                    in_cell = false;
                } else if name_str.contains("table:table-row") {
                    if !row_cells.is_empty() {
                        result.push_str(&row_cells.join("\t"));
                        result.push('\n');
                    }
                    row_cells.clear();
                }
            }
            Ok(Event::Text(e)) => {
                if in_cell {
                    let text = e.unescape().unwrap_or_default();
                    current_cell_text.push_str(&text);
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

    regex::Regex::new(r"[\s]+")
        .unwrap()
        .replace_all(&text, " ")
        .trim()
        .to_string()
}

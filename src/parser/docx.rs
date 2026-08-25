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
    // Word 域代码（HYPERLINK/PAGEREF 等指令）存放在 w:instrText 中，域标记为 w:fldChar，
    // 修订删除文本为 w:delText；这些都不是正文，需在删除普通标签前先整体移除。
    let text = regex::Regex::new(r"(?s)<w:instrText[^>]*>.*?</w:instrText>")
        .unwrap()
        .replace_all(xml, "")
        .to_string();
    let text = regex::Regex::new(r"<w:fldChar[^>]*/?>")
        .unwrap()
        .replace_all(&text, "")
        .to_string();
    let text = regex::Regex::new(r"(?s)<w:delText[^>]*>.*?</w:delText>")
        .unwrap()
        .replace_all(&text, "")
        .to_string();

    // Remove XML tags and decode HTML entities
    let text = regex::Regex::new(r"<[^>]+>")
        .unwrap()
        .replace_all(&text, "")
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

#[cfg(test)]
mod tests {
    use super::extract_text_from_xml;

    #[test]
    fn test_extract_text_skips_field_codes() {
        let xml = r#"<w:document><w:body>
            <w:p>
                <w:r><w:fldChar w:fldCharType="begin"/></w:r>
                <w:r><w:instrText xml:space="preserve"> HYPERLINK \l "_Toc12320" </w:instrText></w:r>
                <w:r><w:fldChar w:fldCharType="separate"/></w:r>
                <w:r><w:t>0.1 总则</w:t></w:r>
                <w:r><w:instrText> PAGEREF _Toc12320 \h 52 </w:instrText></w:r>
                <w:r><w:fldChar w:fldCharType="end"/></w:r>
            </w:p>
            <w:p>
                <w:r><w:delText>deleted draft text</w:delText><w:t>正文内容</w:t></w:r>
            </w:p>
        </w:body></w:document>"#;
        let text = extract_text_from_xml(xml);
        assert!(text.contains("0.1 总则"));
        assert!(text.contains("正文内容"));
        assert!(!text.contains("HYPERLINK"));
        assert!(!text.contains("PAGEREF"));
        assert!(!text.contains("_Toc12320"));
        assert!(!text.contains("deleted draft"));
    }
}

use super::encoding::read_text;
use super::FileParser;
use anyhow::Result;
use pulldown_cmark::{Parser, Options, html};
use std::path::Path;

pub struct MdParser;

impl FileParser for MdParser {
    fn can_parse(&self, path: &Path) -> bool {
        path.extension()
            .map(|ext| ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown"))
            .unwrap_or(false)
    }

    fn parse(&self, path: &Path) -> Result<String> {
        let content = read_text(path)?;

        // Use pulldown-cmark to parse Markdown and extract text
        let mut options = Options::empty();
        options.insert(Options::ENABLE_STRIKETHROUGH);
        options.insert(Options::ENABLE_TABLES);

        let parser = Parser::new_ext(&content, options);

        // Convert to HTML first, then strip tags to get plain text
        let mut html_output = String::new();
        html::push_html(&mut html_output, parser);

        // Strip HTML tags to get plain text
        let text = strip_html_tags(&html_output);
        Ok(text)
    }
}

fn strip_html_tags(html: &str) -> String {
    // Simple HTML tag stripping
    let text = regex::Regex::new(r"<[^>]+>")
        .unwrap()
        .replace_all(html, "")
        .to_string();

    // Decode HTML entities
    let text = text
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
        .replace("&#39;", "'")
        .replace("&#x27;", "'");

    // Clean up extra whitespace
    regex::Regex::new(r"\s+")
        .unwrap()
        .replace_all(&text, " ")
        .trim()
        .to_string()
}

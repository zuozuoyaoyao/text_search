pub mod csv;
pub mod docx;
pub mod excel;
pub mod md;
pub mod odp;
pub mod ods;
pub mod odt;
pub mod pdf;
pub mod pptx;
pub mod rtf;
pub mod txt;

#[cfg(test)]
mod tests;

use anyhow::Result;
use std::path::Path;

pub trait FileParser {
    fn can_parse(&self, path: &Path) -> bool;
    fn parse(&self, path: &Path) -> Result<String>;
}

pub struct ParserManager {
    parsers: Vec<Box<dyn FileParser + Send + Sync>>,
}

impl ParserManager {
    pub fn new() -> Self {
        Self {
            parsers: vec![
                Box::new(docx::DocxParser),
                Box::new(pptx::PptxParser),
                Box::new(excel::ExcelParser),
                Box::new(pdf::PdfParser),
                Box::new(csv::CsvParser),
                Box::new(txt::TxtParser),
                Box::new(md::MdParser),
                Box::new(rtf::RtfParser),
                Box::new(odt::OdtParser),
                Box::new(ods::OdsParser),
                Box::new(odp::OdpParser),
            ],
        }
    }

    pub fn parse(&self, path: &Path) -> Result<String> {
        for parser in &self.parsers {
            if parser.can_parse(path) {
                return parser.parse(path);
            }
        }
        Err(anyhow::anyhow!("No parser found for file: {:?}", path))
    }
}

impl Default for ParserManager {
    fn default() -> Self {
        Self::new()
    }
}

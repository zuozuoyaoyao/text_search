use super::FileParser;
use anyhow::{Context, Result};
use calamine::{open_workbook, Reader, Xls, Xlsx};
use std::path::Path;

pub struct ExcelParser;

impl FileParser for ExcelParser {
    fn can_parse(&self, path: &Path) -> bool {
        path.extension()
            .map(|ext| {
                ext.eq_ignore_ascii_case("xlsx") || ext.eq_ignore_ascii_case("xls")
            })
            .unwrap_or(false)
    }

    fn parse(&self, path: &Path) -> Result<String> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        if ext.eq_ignore_ascii_case("xlsx") {
            self.parse_xlsx(path)
        } else if ext.eq_ignore_ascii_case("xls") {
            self.parse_xls(path)
        } else {
            Err(anyhow::anyhow!("Unsupported Excel format"))
        }
    }
}

impl ExcelParser {
    fn parse_xlsx(&self, path: &Path) -> Result<String> {
        let mut workbook: Xlsx<_> = open_workbook(path)
            .context("Failed to open xlsx file")?;

        self.read_workbook(&mut workbook)
    }

    fn parse_xls(&self, path: &Path) -> Result<String> {
        let mut workbook: Xls<_> = open_workbook(path)
            .context("Failed to open xls file")?;

        self.read_workbook(&mut workbook)
    }

    fn read_workbook<R, Rs>(&self, workbook: &mut R) -> Result<String>
    where
        R: Reader<Rs>,
        Rs: std::io::Read + std::io::Seek,
    {
        let mut all_text = String::new();

        let sheet_names = workbook.sheet_names().clone();

        for sheet_name in sheet_names {
            if let Ok(range) = workbook.worksheet_range(&sheet_name) {
                for row in range.rows() {
                    let row_text: String = row
                        .iter()
                        .map(|cell| {
                            cell.to_string()
                                .replace('\n', " ")
                                .replace('\r', "")
                        })
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                        .join("\t");

                    if !row_text.trim().is_empty() {
                        if !all_text.is_empty() {
                            all_text.push('\n');
                        }
                        all_text.push_str(&row_text);
                    }
                }
            }
        }

        Ok(all_text)
    }
}

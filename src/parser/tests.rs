#[cfg(test)]
mod tests {
    use crate::parser::{ParserManager, FileParser};
    use crate::parser::txt::TxtParser;
    use crate::parser::csv::CsvParser;
    use crate::parser::md::MdParser;
    use crate::parser::rtf::RtfParser;
    use crate::parser::odt::OdtParser;
    use crate::parser::ods::OdsParser;
    use crate::parser::odp::OdpParser;
    use crate::parser::docx::DocxParser;
    use crate::parser::pptx::PptxParser;
    use crate::parser::excel::ExcelParser;
    use crate::parser::pdf::PdfParser;
    use std::path::PathBuf;

    fn get_test_file_path(filename: &str) -> PathBuf {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("test_files");
        path.push(filename);
        path
    }

    #[test]
    fn test_txt_parser() {
        let parser = TxtParser;
        let path = get_test_file_path("test.txt");
        
        assert!(parser.can_parse(&path));
        
        let result = parser.parse(&path);
        assert!(result.is_ok());
        
        let content = result.unwrap();
        assert!(content.contains("plain text file"));
        assert!(content.contains("search"));
    }

    #[test]
    fn test_csv_parser() {
        let parser = CsvParser;
        let path = get_test_file_path("test.csv");
        
        assert!(parser.can_parse(&path));
        
        let result = parser.parse(&path);
        assert!(result.is_ok());
        
        let content = result.unwrap();
        assert!(content.contains("Alice"));
        assert!(content.contains("Bob"));
    }

    #[test]
    fn test_md_parser() {
        let parser = MdParser;
        let path = get_test_file_path("test.md");
        
        assert!(parser.can_parse(&path));
        
        let result = parser.parse(&path);
        assert!(result.is_ok());
        
        let content = result.unwrap();
        assert!(content.contains("Markdown"));
        assert!(content.contains("test"));
    }

    #[test]
    fn test_rtf_parser() {
        let parser = RtfParser;
        let path = get_test_file_path("test.rtf");
        
        assert!(parser.can_parse(&path));
        
        let result = parser.parse(&path);
        assert!(result.is_ok());
        
        let content = result.unwrap();
        println!("RTF parsed content: {}", content);
        // RTF parser should extract some text content
        assert!(!content.is_empty());
    }

    #[test]
    fn test_odt_parser() {
        let parser = OdtParser;
        let path = get_test_file_path("test.odt");
        
        assert!(parser.can_parse(&path));
        
        let result = parser.parse(&path);
        assert!(result.is_ok());
        
        let content = result.unwrap();
        assert!(content.contains("ODT"));
        assert!(content.contains("test"));
    }

    #[test]
    fn test_ods_parser() {
        let parser = OdsParser;
        let path = get_test_file_path("test.ods");
        
        assert!(parser.can_parse(&path));
        
        let result = parser.parse(&path);
        assert!(result.is_ok());
        
        let content = result.unwrap();
        assert!(content.contains("Alice") || content.contains("Name"));
    }

    #[test]
    fn test_odp_parser() {
        let parser = OdpParser;
        let path = get_test_file_path("test.odp");

        assert!(parser.can_parse(&path));

        let result = parser.parse(&path);
        assert!(result.is_ok());

        let content = result.unwrap();
        assert!(content.contains("ODP"));
        assert!(content.contains("test"));
    }

    #[test]
    fn test_docx_parser() {
        let parser = DocxParser;
        let path = get_test_file_path("test.docx");

        assert!(parser.can_parse(&path));

        let result = parser.parse(&path);
        assert!(result.is_ok());

        let content = result.unwrap();
        // Check for Chinese content and table data
        assert!(content.contains("docx") || content.contains("姓名") || content.contains("张三"));
    }

    #[test]
    fn test_pptx_parser() {
        let parser = PptxParser;
        let path = get_test_file_path("test.pptx");

        assert!(parser.can_parse(&path));

        let result = parser.parse(&path);
        assert!(result.is_ok());

        let content = result.unwrap();
        // PPTX should contain some text content
        assert!(!content.is_empty());
    }

    #[test]
    fn test_excel_parser_xlsx() {
        let parser = ExcelParser;
        let path = get_test_file_path("test.xlsx");

        assert!(parser.can_parse(&path));

        let result = parser.parse(&path);
        assert!(result.is_ok());

        let content = result.unwrap();
        // Check for spreadsheet data
        assert!(content.contains("姓名") || content.contains("张三"));
    }

    #[test]
    fn test_pdf_parser() {
        let parser = PdfParser;
        let path = get_test_file_path("test.pdf");

        assert!(parser.can_parse(&path));

        let result = parser.parse(&path);
        assert!(result.is_ok());

        // PDF extraction returns empty string currently
        // This test ensures the parser doesn't crash
        let _content = result.unwrap();
        println!("PDF parsed content: {}", _content);
    }

    #[test]
    fn test_parser_manager() {
        let manager = ParserManager::new();
        let path = get_test_file_path("test.txt");
        
        let result = manager.parse(&path);
        assert!(result.is_ok());
        
        let content = result.unwrap();
        assert!(content.contains("plain text file"));
    }

    #[test]
    fn test_parser_manager_md() {
        let manager = ParserManager::new();
        let path = get_test_file_path("test.md");
        
        let result = manager.parse(&path);
        assert!(result.is_ok());
        
        let content = result.unwrap();
        assert!(content.contains("Markdown"));
    }

    #[test]
    fn test_parser_manager_unknown_extension() {
        let manager = ParserManager::new();
        let path = PathBuf::from("/tmp/test.xyz");

        let result = manager.parse(&path);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No parser found"));
    }

    #[test]
    fn test_parser_manager_docx() {
        let manager = ParserManager::new();
        let path = get_test_file_path("test.docx");

        let result = manager.parse(&path);
        assert!(result.is_ok());

        let content = result.unwrap();
        assert!(content.contains("docx") || content.contains("姓名") || content.contains("张三"));
    }

    #[test]
    fn test_parser_manager_pptx() {
        let manager = ParserManager::new();
        let path = get_test_file_path("test.pptx");

        let result = manager.parse(&path);
        assert!(result.is_ok());

        let content = result.unwrap();
        assert!(!content.is_empty());
    }

    #[test]
    fn test_parser_manager_excel() {
        let manager = ParserManager::new();
        let path = get_test_file_path("test.xlsx");

        let result = manager.parse(&path);
        assert!(result.is_ok());

        let content = result.unwrap();
        assert!(content.contains("姓名") || content.contains("张三"));
    }

    #[test]
    fn test_parser_manager_pdf() {
        let manager = ParserManager::new();
        let path = get_test_file_path("test.pdf");

        let result = manager.parse(&path);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parser_manager_rtf() {
        let manager = ParserManager::new();
        let path = get_test_file_path("test.rtf");

        let result = manager.parse(&path);
        assert!(result.is_ok());

        let content = result.unwrap();
        assert!(!content.is_empty());
    }

    #[test]
    fn test_parser_manager_odt() {
        let manager = ParserManager::new();
        let path = get_test_file_path("test.odt");

        let result = manager.parse(&path);
        assert!(result.is_ok());

        let content = result.unwrap();
        assert!(content.contains("ODT"));
    }

    #[test]
    fn test_parser_manager_ods() {
        let manager = ParserManager::new();
        let path = get_test_file_path("test.ods");

        let result = manager.parse(&path);
        assert!(result.is_ok());

        let content = result.unwrap();
        assert!(content.contains("Alice") || content.contains("Name") || content.contains("姓名"));
    }

    #[test]
    fn test_parser_manager_odp() {
        let manager = ParserManager::new();
        let path = get_test_file_path("test.odp");

        let result = manager.parse(&path);
        assert!(result.is_ok());

        let content = result.unwrap();
        assert!(!content.is_empty());
    }

    #[test]
    fn test_parser_manager_csv() {
        let manager = ParserManager::new();
        let path = get_test_file_path("test.csv");

        let result = manager.parse(&path);
        assert!(result.is_ok());

        let content = result.unwrap();
        assert!(content.contains("Alice") || content.contains("name"));
    }
}

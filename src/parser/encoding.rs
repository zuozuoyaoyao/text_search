use anyhow::{Context, Result};
use std::path::Path;

/// 读取文本文件内容，自动处理常见编码：
/// 1. UTF-32LE/BE BOM（防止被 UTF-16 BOM 误判）
/// 2. UTF-8 / ASCII 快路径（str::from_utf8，零开销）
/// 3. UTF-8 / UTF-16LE / UTF-16BE BOM（encoding_rs）
/// 4. 其他遗留编码（GBK/GB18030、Big5、Shift_JIS 等）由 chardetng 启发式探测
///
/// 个别无法解码的字节会被替换为 U+FFFD，不影响索引与搜索。
pub fn read_text(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path).with_context(|| format!("Failed to read file: {:?}", path))?;

    if bytes.starts_with(&[0xFF, 0xFE, 0x00, 0x00]) {
        return decode_u32(&bytes[4..], true);
    }
    if bytes.starts_with(&[0x00, 0x00, 0xFE, 0xFF]) {
        return decode_u32(&bytes[4..], false);
    }

    if let Some((enc, bom_len)) = encoding_rs::Encoding::for_bom(&bytes) {
        return Ok(enc
            .decode_without_bom_handling(&bytes[bom_len..])
            .0
            .into_owned());
    }

    if let Ok(s) = std::str::from_utf8(&bytes) {
        return Ok(s.to_string());
    }

    let mut detector = chardetng::EncodingDetector::new();
    detector.feed(&bytes, true);
    let enc = detector.guess(None, true);
    Ok(enc.decode(&bytes).0.into_owned())
}

fn decode_u32(bytes: &[u8], little_endian: bool) -> Result<String> {
    let units: Vec<u16> = bytes
        .chunks_exact(4)
        .map(|c| {
            if little_endian {
                u16::from_le_bytes([c[0], c[1]])
            } else {
                u16::from_be_bytes([c[0], c[1]])
            }
        })
        .collect();
    Ok(String::from_utf16_lossy(&units))
}

#[cfg(test)]
mod tests {
    use super::read_text;
    use std::io::Write;
    use std::path::PathBuf;

    fn temp_file(name: &str, bytes: &[u8]) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("ts_encoding_test_{}_{}", std::process::id(), name));
        let mut f = std::fs::File::create(&p).unwrap();
        f.write_all(bytes).unwrap();
        p
    }

    #[test]
    fn test_utf8_ascii() {
        let p = temp_file("utf8.txt", b"hello \xe4\xbd\xa0\xe5\xa5\xbd\nplain text");
        let s = read_text(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert_eq!(s, "hello 你好\nplain text");
    }

    #[test]
    fn test_gbk_chinese() {
        // "中文测试" 的 GBK 编码字节
        let p = temp_file("gbk.txt", b"\xd6\xd0\xce\xc4\xb2\xe2\xca\xd4\r\nsecond line");
        let s = read_text(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert!(s.contains("中文测试"), "got: {}", s);
        assert!(s.contains("second line"));
    }

    #[test]
    fn test_utf16le_bom() {
        let mut bytes = vec![0xFF, 0xFE];
        let text = "你好，世界 hello";
        for u in text.encode_utf16() {
            bytes.extend_from_slice(&u.to_le_bytes());
        }
        let p = temp_file("utf16.txt", &bytes);
        let s = read_text(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert_eq!(s, text);
    }

    #[test]
    fn test_utf16be_bom() {
        let mut bytes = vec![0xFE, 0xFF];
        let text = "hello world";
        for u in text.encode_utf16() {
            bytes.extend_from_slice(&u.to_be_bytes());
        }
        let p = temp_file("utf16be.txt", &bytes);
        let s = read_text(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert_eq!(s, text);
    }

    #[test]
    fn test_utf8_bom() {
        let p = temp_file("utf8bom.txt", b"\xef\xbb\xbfhello bom");
        let s = read_text(&p).unwrap();
        std::fs::remove_file(&p).unwrap();
        assert_eq!(s, "hello bom");
    }
}

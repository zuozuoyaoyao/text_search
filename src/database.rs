use anyhow::{Context, Result};
use duckdb::{params, Connection};
use std::path::Path;
use std::sync::Mutex;

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn new(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)
            .with_context(|| format!("Failed to open database at: {:?}", db_path))?;

        let db = Database {
            conn: Mutex::new(conn),
        };

        db.create_tables()?;

        Ok(db)
    }

    fn create_tables(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS file (
                name TEXT,
                abs_path TEXT PRIMARY KEY,
                content TEXT,
                md5 TEXT,
                last_modified_time BIGINT,
                last_indextime TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        ).context("Failed to create file table")?;

        Ok(())
    }

    /// 获取锁保护的连接引用
    fn with_conn<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Connection) -> Result<T>,
    {
        let conn = self.conn.lock().unwrap();
        f(&conn)
    }

    pub fn insert_or_update_file(&self, name: &str, abs_path: &str, content: &str, md5: &str, last_modified_time: i64) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute(
                "INSERT OR REPLACE INTO file (name, abs_path, content, md5, last_modified_time, last_indextime)
                 VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP)",
                params![name, abs_path, content, md5, last_modified_time],
            ).context("Failed to insert/update file")
        })?;

        Ok(())
    }

    pub fn delete_file(&self, abs_path: &str) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute(
                "DELETE FROM file WHERE abs_path = ?",
                params![abs_path],
            ).context("Failed to delete file")
        })?;

        Ok(())
    }

    pub fn search(&self, keyword: &str, context_length: usize) -> Result<Vec<SearchResult>> {
        let conn = self.conn.lock().unwrap();
        let keyword_pattern = format!("%{}%", keyword.replace('%', "\\%"));
        let half_context = context_length / 2;

        let mut stmt = conn.prepare_cached(
            "SELECT name, abs_path, content, last_modified_time
             FROM file
             WHERE content LIKE ?
             ORDER BY last_modified_time DESC",
        ).context("Failed to prepare search statement")?;

        let mut results = Vec::new();
        let mut rows = stmt.query(params![&keyword_pattern])
            .context("Failed to execute search query")?;

        while let Some(row) = rows.next()? {
            let name: String = row.get(0)?;
            let abs_path: String = row.get(1)?;
            let content: String = row.get(2)?;
            let last_modified_time: i64 = row.get(3)?;

            // Find the keyword position and extract context using char indices
            let keyword_lower = keyword.to_lowercase();
            let content_lower = content.to_lowercase();

            if let Some(byte_pos) = content_lower.find(&keyword_lower) {
                // Convert byte position to char position
                let char_pos = content_lower[..byte_pos].chars().count();
                let keyword_char_len = keyword_lower.chars().count();

                let start_char = char_pos.saturating_sub(half_context);
                let end_char = (char_pos + keyword_char_len + half_context).min(content.chars().count());

                // Convert char positions back to byte positions
                let start_byte = content.char_indices().nth(start_char).map(|(i, _)| i).unwrap_or(0);
                let end_byte = content.char_indices().nth(end_char).map(|(i, _)| i).unwrap_or(content.len());

                let context = content[start_byte..end_byte].to_string();
                let is_start = start_char == 0;
                let is_end = end_char == content.chars().count();

                results.push(SearchResult {
                    name,
                    path: abs_path,
                    context,
                    is_start,
                    is_end,
                    last_modified_time,
                });
            }
        }

        Ok(results)
    }

    #[allow(dead_code)]
    pub fn execute_custom_sql(&self, sql: &str) -> Result<Vec<CustomResult>> {
        let conn = self.conn.lock().unwrap();
        // DuckDB's Rust binding - we need to use prepare + query approach
        let mut stmt = conn.prepare(sql)
            .context("Failed to prepare custom SQL statement")?;

        // Execute and get rows
        let mut rows = stmt.query([])
            .context("Failed to execute custom SQL query")?;

        let mut results = Vec::new();

        // We need to get column info after executing, but duckdb's API makes this tricky
        // For now, we'll use a simpler approach with generic column names
        // and determine column count from the first row
        let mut first_row_processed = false;
        let column_count = rows.as_ref().unwrap().column_count();
        let mut column_names: Vec<String> = vec![];

        while let Some(row) = rows.next()? {
            if !first_row_processed {
                for i in 0..column_count {
                    let name = row.as_ref().column_name(i)
                        .unwrap_or(&format!("column_{}", i))
                        .to_string();
                    column_names.push(name);
                }
                first_row_processed = true;
            }

            let mut values = Vec::new();
            for i in 0..column_count {
                let value_str = row.get::<_, Option<String>>(i)
                    .ok()
                    .flatten()
                    .or_else(|| row.get::<_, Option<i64>>(i).ok().flatten().map(|v| v.to_string()))
                    .or_else(|| row.get::<_, Option<i32>>(i).ok().flatten().map(|v| v.to_string()))
                    .or_else(|| row.get::<_, Option<f64>>(i).ok().flatten().map(|v| v.to_string()))
                    .unwrap_or_else(|| "NULL".to_string());
                values.push(value_str);
            }

            results.push(CustomResult {
                columns: column_names.clone(),
                values,
            });
        }

        Ok(results)
    }

    #[allow(dead_code)]
    pub fn get_file_md5(&self, abs_path: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare_cached(
            "SELECT md5 FROM file WHERE abs_path = ?",
        )?;

        let mut rows = stmt.query(params![abs_path])?;

        if let Some(row) = rows.next()? {
            let md5: Option<String> = row.get(0)?;
            Ok(md5)
        } else {
            Ok(None)
        }
    }

    pub fn get_file_info(&self, abs_path: &str) -> Result<Option<(String, i64)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare_cached(
            "SELECT md5, last_modified_time FROM file WHERE abs_path = ?",
        )?;

        let mut rows = stmt.query(params![abs_path])?;

        if let Some(row) = rows.next()? {
            let md5: Option<String> = row.get(0)?;
            let last_modified_time: Option<i64> = row.get(1)?;
            match (md5, last_modified_time) {
                (Some(m), Some(t)) => Ok(Some((m, t))),
                _ => Ok(None),
            }
        } else {
            Ok(None)
        }
    }

    #[allow(dead_code)]
    pub fn get_all_files(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare_cached(
            "SELECT abs_path FROM file",
        )?;

        let mut paths = Vec::new();
        let mut rows = stmt.query([])?;

        while let Some(row) = rows.next()? {
            let path: String = row.get(0)?;
            paths.push(path);
        }

        Ok(paths)
    }

    pub fn clear_all(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM file", [])
            .context("Failed to clear all files")?;
        Ok(())
    }

    /// Delete all files whose abs_path starts with the given prefix (e.g. a removed watch dir).
    /// Returns the number of deleted rows.
    pub fn delete_files_by_prefix(&self, prefix: &str) -> Result<u64> {
        self.with_conn(|conn| {
            // 匹配路径本身（罕见情况）及路径下的所有文件（兼容 / 和 \ 路径分隔符）
            let rows = conn.execute(
                "DELETE FROM file WHERE abs_path = ?1 OR abs_path LIKE ?1 || '/%' OR abs_path LIKE ?1 || '\\%'",
                params![prefix],
            ).context("Failed to delete files by prefix")?;
            Ok(rows as u64)
        })
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SearchResult {
    pub name: String,
    pub path: String,
    pub context: String,
    pub is_start: bool,
    pub is_end: bool,
    pub last_modified_time: i64,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct CustomResult {
    pub columns: Vec<String>,
    pub values: Vec<String>,
}

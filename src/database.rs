use anyhow::{Context, Result};
use rusqlite::{params, params_from_iter, Connection, Statement};
use std::path::Path;
use std::sync::Mutex;

const INCREMENTAL_VACUUM_MIN_FREE_PAGES: i64 = 64;
const INCREMENTAL_VACUUM_MAX_PAGES: i64 = 4096;

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn new(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)
            .with_context(|| format!("Failed to open database at: {:?}", db_path))?;
        // Keep WAL for the lifetime of the connection. Existing databases are
        // migrated below because auto_vacuum changes require one VACUUM.
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA auto_vacuum=INCREMENTAL;")
            .context("Failed to set SQLite pragmas")?;

        let auto_vacuum: i64 = conn
            .query_row("PRAGMA auto_vacuum", [], |row| row.get(0))
            .context("Failed to read SQLite auto_vacuum mode")?;
        if auto_vacuum != 2 {
            tracing::info!(
                "Migrating SQLite auto_vacuum mode from {} to incremental",
                auto_vacuum
            );
            // This is a one-time migration. VACUUM is safe while staying in WAL.
            conn.execute_batch("PRAGMA auto_vacuum=INCREMENTAL; VACUUM;")
                .context("Failed to enable incremental auto_vacuum")?;
        }

        let db = Database {
            conn: Mutex::new(conn),
        };

        db.create_tables()?;
        db.migrate()?;

        Ok(db)
    }

    fn create_tables(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS file (
                name TEXT,
                abs_path TEXT PRIMARY KEY,
                md5 TEXT,
                last_modified_time TIMESTAMP,
                content TEXT,
                last_indextime TIMESTAMP DEFAULT (datetime('now','localtime')),
                last_seen TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS config (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS bookmark_category (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                sort_order INTEGER DEFAULT 0,
                created_at TIMESTAMP DEFAULT (datetime('now','localtime'))
            );

            CREATE TABLE IF NOT EXISTS bookmark (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                category_id INTEGER NOT NULL,
                abs_path TEXT NOT NULL,
                name TEXT NOT NULL,
                file_size INTEGER,
                last_modified_time TEXT,
                created_at TIMESTAMP DEFAULT (datetime('now','localtime')),
                FOREIGN KEY (category_id) REFERENCES bookmark_category(id) ON DELETE CASCADE,
                UNIQUE(category_id, abs_path)
            );"
        ).context("Failed to create tables")?;

        Ok(())
    }

    fn migrate(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());

        conn.execute_batch(
            "DROP TRIGGER IF EXISTS file_ai;
             DROP TRIGGER IF EXISTS file_ad;
             DROP TRIGGER IF EXISTS file_au;
             DROP TABLE IF EXISTS file_fts;",
        )
        .context("Failed to drop legacy FTS triggers/tables")?;

        let has_content: bool = conn
            .prepare("SELECT content FROM file LIMIT 0")
            .map(|_| true)
            .unwrap_or(false);
        if !has_content {
            conn.execute_batch("ALTER TABLE file ADD COLUMN content TEXT;")
                .context("Failed to add content column")?;
            tracing::info!("Migrated database: added content column");
        }

        let has_config: bool = conn
            .prepare("SELECT key FROM config LIMIT 0")
            .map(|_| true)
            .unwrap_or(false);
        if !has_config {
            conn.execute_batch("CREATE TABLE IF NOT EXISTS config (key TEXT PRIMARY KEY, value TEXT NOT NULL);")
                .context("Failed to create config table")?;
            tracing::info!("Migrated database: added config table");
        }

        let has_last_seen: bool = conn
            .prepare("SELECT last_seen FROM file LIMIT 0")
            .map(|_| true)
            .unwrap_or(false);
        if !has_last_seen {
            conn.execute_batch("ALTER TABLE file ADD COLUMN last_seen TIMESTAMP;")
                .context("Failed to add last_seen column")?;
            tracing::info!("Migrated database: added last_seen column");
        }

        let has_file_size: bool = conn
            .prepare("SELECT file_size FROM file LIMIT 0")
            .map(|_| true)
            .unwrap_or(false);
        if !has_file_size {
            conn.execute_batch("ALTER TABLE file ADD COLUMN file_size INTEGER;")
                .context("Failed to add file_size column")?;
            tracing::info!("Migrated database: added file_size column");
        }

        let need_time_migration: bool = {
            let result: Option<String> = conn
                .query_row("SELECT CAST(last_modified_time AS TEXT) FROM file LIMIT 1", [], |row| row.get(0))
                .ok();
            match result {
                Some(s) => s.parse::<i64>().is_ok() || s.parse::<f64>().is_ok(),
                None => false,
            }
        };

        if need_time_migration {
            conn.execute_batch(
                "UPDATE file SET last_modified_time = 
                    datetime(last_modified_time / 1000, 'unixepoch', 'localtime')
                 WHERE last_modified_time IS NOT NULL AND CAST(last_modified_time AS TEXT) GLOB '[0-9]*';"
            ).context("Failed to migrate last_modified_time")?;
            tracing::info!("Migrated last_modified_time from epoch millis to local timestamp");
        }

        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_file_mtime_path ON file(last_modified_time, abs_path);
             CREATE INDEX IF NOT EXISTS idx_file_name_path ON file(name, abs_path);
             CREATE INDEX IF NOT EXISTS idx_file_size_path ON file(file_size, abs_path);"
        )
        .context("Failed to create file indexes")?;

        Ok(())
    }

    fn with_conn<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Connection) -> Result<T>,
    {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        f(&conn)
    }

    /// 退出前调用：执行 WAL checkpoint，将数据从 WAL 文件刷入主库。
    pub fn close(&self) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
                .context("Failed to checkpoint WAL on close")
        })
    }

    /// 开启事务，用于批量索引提交（每 N 个文件提交一次，减少 fsync）。
    pub fn begin_transaction(&self) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute_batch("BEGIN")
                .context("Failed to begin transaction")
        })
    }

    pub fn commit_transaction(&self) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute_batch("COMMIT")
                .context("Failed to commit transaction")
        })
    }

    pub fn rollback_transaction(&self) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute_batch("ROLLBACK")
                .context("Failed to rollback transaction")
        })
    }

    pub fn insert_or_update_file(
        &self,
        name: &str,
        abs_path: &str,
        last_modified_time: &str,
        file_size: i64,
        content: &str,
    ) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute(
                "INSERT OR REPLACE INTO file (name, abs_path, last_modified_time, file_size, content, last_indextime, last_seen)
                 VALUES (?, ?, ?, ?, ?, datetime('now','localtime'), datetime('now','localtime'))",
                params![name, abs_path, last_modified_time, file_size, content],
            )
            .context("Failed to insert/update file")
        })?;

        Ok(())
    }

    pub fn delete_file(&self, abs_path: &str) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute(
                "DELETE FROM file WHERE abs_path = ?",
                params![abs_path],
            )
            .context("Failed to delete file")
        })?;

        Ok(())
    }

    pub fn execute_custom_sql(&self, sql: &str) -> Result<Vec<CustomResult>> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let mut stmt = conn.prepare(sql).context("Failed to prepare custom SQL statement")?;
        stmt_to_result(&mut stmt, params![]).map(|r| r.rows)
    }

    pub fn get_file_info(&self, abs_path: &str) -> Result<Option<(String, i64)>> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let mut stmt = conn.prepare_cached(
            "SELECT last_modified_time, COALESCE(file_size, 0) FROM file WHERE abs_path = ?",
        )?;

        let result = stmt
            .query_row(params![abs_path], |row| {
                let time: Option<String> = row.get(0)?;
                let size: i64 = row.get(1)?;
                Ok((time, size))
            })
            .ok();

        match result {
            Some((Some(t), s)) => Ok(Some((t, s))),
            _ => Ok(None),
        }
    }

    pub fn get_content(&self, abs_path: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let mut stmt = conn.prepare_cached(
            "SELECT content FROM file WHERE abs_path = ?",
        )?;
        let result = stmt
            .query_row(params![abs_path], |row| {
                let content: Option<String> = row.get(0)?;
                Ok(content)
            })
            .ok()
            .flatten();
        Ok(result)
    }

    /// 返回文件预览信息：name, file_size, last_modified_time, content 前缀(limit 字符), 是否截断。
    pub fn get_file_content_preview(
        &self,
        abs_path: &str,
        limit: usize,
    ) -> Result<Option<(String, Option<i64>, Option<String>, String, bool)>> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare_cached(
                "SELECT name, file_size, last_modified_time, content FROM file WHERE abs_path = ?",
            )?;
            let result = stmt
                .query_row(params![abs_path], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<i64>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                    ))
                })
                .ok();
            let Some((name, file_size, mtime, content_opt)) = result else {
                return Ok(None);
            };
            let (content, truncated) = match content_opt {
                Some(c) if c.chars().count() > limit => {
                    let prefix: String = c.chars().take(limit).collect();
                    (prefix, true)
                }
                Some(c) => (c, false),
                None => (String::new(), false),
            };
            Ok(Some((name, file_size, mtime, content, truncated)))
        })
    }

    pub fn search_like(
        &self,
        keywords: &[String],
        mode: &str,
        filters: &SearchFilters,
        last_cursor: Option<&SearchCursor>,
        sort_by: &str,
        page_size: usize,
        context_length: usize,
        name_only: bool,
    ) -> Result<(Vec<LikeSearchHit>, Option<SearchCursor>, bool)> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());

        let hit_column = if name_only { "name" } else { "content" };
        let conditions: Vec<String> = keywords
            .iter()
            .filter(|k| !k.is_empty())
            .map(|_| format!("{} LIKE ?", hit_column))
            .collect();

        if conditions.is_empty() {
            return Ok((Vec::new(), None, false));
        }

        let joiner = if mode.eq_ignore_ascii_case("AND") {
            " AND "
        } else {
            " OR "
        };
        let keyword_clause = format!("({})", conditions.join(joiner));

        let mut params: Vec<rusqlite::types::Value> = keywords
            .iter()
            .filter(|k| !k.is_empty())
            .map(|k| {
                rusqlite::types::Value::Text(format!("%{}%", k.replace('%', "\\%").replace('_', "\\_")))
            })
            .collect();

        let filter_clauses = build_filter_sql(filters, &mut params);

        let mut where_parts = vec![keyword_clause];
        where_parts.extend(filter_clauses);

        let (order_by, sort_col, sort_desc) = parse_sort_by(sort_by);

        if let Some(cursor) = last_cursor {
            let op_primary = if sort_desc { "<" } else { ">" };
            let cursor_clause = format!(
                "({} {} ? OR ({} = ? AND abs_path > ?))",
                sort_col, op_primary, sort_col
            );
            params.push(rusqlite::types::Value::Text(cursor.sort_value.clone()));
            params.push(rusqlite::types::Value::Text(cursor.sort_value.clone()));
            params.push(rusqlite::types::Value::Text(cursor.path.clone()));
            where_parts.push(cursor_clause);
        }
        let where_clause = where_parts.join(" AND ");

        let sql = format!(
            "SELECT abs_path, name, content, last_modified_time, file_size FROM file
             WHERE {} ORDER BY {} LIMIT {}",
            where_clause,
            order_by,
            page_size + 1
        );

        let mut stmt = conn.prepare(&sql)?;
        let mut results = Vec::new();

        let mut rows = stmt.query(params_from_iter(params.iter()))?;
        while let Some(row) = rows.next()? {
            let abs_path: String = row.get(0)?;
            let name: String = row.get(1)?;
            let content: Option<String> = row.get(2)?;
            let last_modified_time: String = row.get(3)?;
            let file_size: i64 = row.get(4)?;

            let context = if name_only {
                match content {
                    Some(ref c) if !c.is_empty() => {
                        content_prefix(c, context_length * 2)
                    }
                    _ => String::new(),
                }
            } else {
                match content {
                    Some(ref c) if !c.is_empty() => {
                        make_snippet(c, keywords, context_length)
                    }
                    _ => String::new(),
                }
            };

            results.push(LikeSearchHit {
                abs_path,
                name,
                context,
                last_modified_time,
                file_size,
            });
        }

        let has_more = results.len() > page_size;
        results.truncate(page_size);
        let next_cursor = results.last().map(|h| SearchCursor {
            sort_value: get_sort_value(h, sort_col),
            path: h.abs_path.clone(),
        });

        Ok((results, next_cursor, has_more))
    }

    pub fn count_like(
        &self,
        keywords: &[String],
        mode: &str,
        filters: &SearchFilters,
        name_only: bool,
    ) -> Result<i64> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());

        let hit_column = if name_only { "name" } else { "content" };
        let conditions: Vec<String> = keywords
            .iter()
            .filter(|k| !k.is_empty())
            .map(|_| format!("{} LIKE ?", hit_column))
            .collect();

        if conditions.is_empty() {
            return Ok(0);
        }

        let joiner = if mode.eq_ignore_ascii_case("AND") {
            " AND "
        } else {
            " OR "
        };
        let keyword_clause = format!("({})", conditions.join(joiner));

        let mut params: Vec<rusqlite::types::Value> = keywords
            .iter()
            .filter(|k| !k.is_empty())
            .map(|k| {
                rusqlite::types::Value::Text(format!("%{}%", k.replace('%', "\\%").replace('_', "\\_")))
            })
            .collect();

        let filter_clauses = build_filter_sql(filters, &mut params);

        let mut where_parts = vec![keyword_clause];
        where_parts.extend(filter_clauses);
        let where_clause = where_parts.join(" AND ");

        let sql = format!(
            "SELECT COUNT(*) FROM file WHERE {}",
            where_clause
        );

        let count: i64 = conn
            .query_row(&sql, params_from_iter(params.iter()), |row| row.get(0))?;
        Ok(count)
    }

    pub fn get_file_types(&self) -> Result<Vec<String>> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare("SELECT name FROM file")?;
            let names: Vec<String> = stmt
                .query_map([], |row| row.get(0))?
                .filter_map(|r| r.ok())
                .collect();

            let mut set = std::collections::BTreeSet::new();
            for name in names {
                if name.starts_with("~$") {
                    continue;
                }
                if let Some(ext) = name.rsplit_once('.') {
                    if !ext.1.is_empty() {
                        set.insert(ext.1.to_lowercase());
                    }
                }
            }
            Ok(set.into_iter().collect())
        })
    }

    pub fn get_file_count(&self) -> Result<i64> {
        self.with_conn(|conn| {
            let count: i64 = conn
                .query_row("SELECT COUNT(*) FROM file", [], |row| row.get(0))
                .unwrap_or(0);
            Ok(count)
        })
    }

    pub fn get_last_index_time(&self) -> Result<Option<String>> {
        self.with_conn(|conn| {
            let result: Option<String> = conn
                .query_row(
                    "SELECT MAX(last_indextime) FROM file",
                    [],
                    |row| row.get(0),
                )
                .ok()
                .flatten();
            Ok(result)
        })
    }

    pub fn clear_all(&self) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute("DELETE FROM file", [])
                .context("Failed to clear all files")?;
            Self::run_incremental_vacuum(conn, None)
                .context("Failed to vacuum after clear")?;
            Ok(())
        })
    }

    pub fn incremental_vacuum(&self) -> Result<()> {
        self.with_conn(|conn| {
            let mut previous: i64 =
                conn.query_row("PRAGMA freelist_count", [], |row| row.get(0))?;
            while previous >= INCREMENTAL_VACUUM_MIN_FREE_PAGES {
                Self::run_incremental_vacuum(conn, Some(INCREMENTAL_VACUUM_MAX_PAGES))?;
                let remaining: i64 =
                    conn.query_row("PRAGMA freelist_count", [], |row| row.get(0))?;
                if remaining >= previous {
                    break;
                }
                previous = remaining;
            }
            Ok(())
        })
    }

    // Reclaim free pages without changing journal mode or closing the connection.
    fn run_incremental_vacuum(conn: &Connection, max_pages: Option<i64>) -> Result<()> {
        let free: i64 = conn.query_row("PRAGMA freelist_count", [], |row| row.get(0))?;
        if free < INCREMENTAL_VACUUM_MIN_FREE_PAGES {
            return Ok(());
        }

        tracing::info!("Reclaiming {} free SQLite pages incrementally", free);

        // incremental_vacuum is a multi-step PRAGMA: each sqlite3_step() reclaims
        // one page and returns SQLITE_ROW with zero columns. execute_batch() only
        // steps such a statement once, so consume all rows explicitly.
        {
            let sql = match max_pages {
                Some(pages) => format!("PRAGMA incremental_vacuum({});", pages),
                None => "PRAGMA incremental_vacuum;".to_string(),
            };
            let mut stmt = conn
                .prepare(&sql)
                .context("Failed to prepare incremental vacuum")?;
            let mut rows = stmt
                .query([])
                .context("Failed to start incremental vacuum")?;
            while rows
                .next()
                .context("Failed while running incremental vacuum")?
                .is_some()
            {}
        }

        let (busy, wal_pages, checkpointed): (i64, i64, i64) = conn
            .query_row("PRAGMA wal_checkpoint(TRUNCATE);", [], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })
            .context("Failed to checkpoint WAL after incremental vacuum")?;
        if busy != 0 {
            tracing::warn!(
                "WAL checkpoint after incremental vacuum was busy: wal_pages={}, checkpointed={}",
                wal_pages,
                checkpointed
            );
        }

        let remaining: i64 =
            conn.query_row("PRAGMA freelist_count", [], |row| row.get(0))?;
        tracing::info!(
            "Incremental vacuum completed: reclaimed={} pages, remaining={} pages",
            free - remaining,
            remaining
        );
        Ok(())
    }

    pub fn add_bookmark_category(&self, name: &str) -> Result<i64> {
        self.with_conn(|conn| {
            conn.execute(
                "INSERT OR IGNORE INTO bookmark_category (name) VALUES (?)",
                params![name],
            )?;
            let id: i64 = conn.query_row(
                "SELECT id FROM bookmark_category WHERE name = ?",
                params![name],
                |row| row.get(0),
            )?;
            Ok(id)
        })
    }

    pub fn rename_bookmark_category(&self, id: i64, name: &str) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE bookmark_category SET name = ? WHERE id = ?",
                params![name, id],
            )?;
            Ok(())
        })
    }

    pub fn delete_bookmark_category(&self, id: i64) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute("DELETE FROM bookmark WHERE category_id = ?", params![id])?;
            conn.execute("DELETE FROM bookmark_category WHERE id = ?", params![id])?;
            Ok(())
        })
    }

    pub fn add_bookmark(
        &self,
        category_id: i64,
        abs_path: &str,
        name: &str,
        file_size: Option<i64>,
        last_modified_time: Option<&str>,
    ) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute(
                "INSERT OR IGNORE INTO bookmark (category_id, abs_path, name, file_size, last_modified_time)
                 VALUES (?, ?, ?, ?, ?)",
                params![category_id, abs_path, name, file_size, last_modified_time],
            )?;
            Ok(())
        })
    }

    pub fn remove_bookmark(&self, id: i64) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute("DELETE FROM bookmark WHERE id = ?", params![id])?;
            Ok(())
        })
    }

    pub fn remove_bookmark_by_path(&self, category_id: i64, abs_path: &str) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute(
                "DELETE FROM bookmark WHERE category_id = ? AND abs_path = ?",
                params![category_id, abs_path],
            )?;
            Ok(())
        })
    }

    pub fn list_bookmarks(&self) -> Result<Vec<BookmarkCategory>> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name FROM bookmark_category ORDER BY sort_order, id"
            )?;
            let categories: Vec<(i64, String)> = stmt
                .query_map([], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                })?
                .filter_map(|r| r.ok())
                .collect();

            let mut result = Vec::new();
            for (cat_id, cat_name) in categories {
                let mut bstmt = conn.prepare(
                    "SELECT id, abs_path, name, file_size, last_modified_time
                     FROM bookmark WHERE category_id = ?
                     ORDER BY created_at DESC"
                )?;
                let bookmarks = bstmt
                    .query_map(params![cat_id], |row| {
                        Ok(BookmarkItem {
                            id: row.get(0)?,
                            abs_path: row.get(1)?,
                            name: row.get(2)?,
                            file_size: row.get(3)?,
                            last_modified_time: row.get(4)?,
                        })
                    })?
                    .filter_map(|r| r.ok())
                    .collect();

                result.push(BookmarkCategory {
                    id: cat_id,
                    name: cat_name,
                    bookmarks,
                });
            }
            Ok(result)
        })
    }

    pub fn delete_files_by_prefix(&self, prefix: &str) -> Result<Vec<String>> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT abs_path FROM file WHERE abs_path = ?1 OR abs_path LIKE ?1 || '/%' OR abs_path LIKE ?1 || '\\%'",
            )?;
            let paths: Vec<String> = stmt
                .query_map(params![prefix], |row| row.get(0))?
                .filter_map(|r| r.ok())
                .collect();

            conn.execute(
                "DELETE FROM file WHERE abs_path = ?1 OR abs_path LIKE ?1 || '/%' OR abs_path LIKE ?1 || '\\%'",
                params![prefix],
            )
            .context("Failed to delete files by prefix")?;

            Ok(paths)
        })
    }

    pub fn touch_file(&self, abs_path: &str) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE file SET last_seen = datetime('now','localtime') WHERE abs_path = ?",
                params![abs_path],
            )
            .context("Failed to touch file")
        })?;
        Ok(())
    }

    pub fn delete_files_before(&self, timestamp: &str) -> Result<Vec<String>> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT abs_path FROM file WHERE last_seen IS NULL OR last_seen < ?",
            )?;
            let paths: Vec<String> = stmt
                .query_map(params![timestamp], |row| row.get(0))?
                .filter_map(|r| r.ok())
                .collect();

            conn.execute(
                "DELETE FROM file WHERE last_seen IS NULL OR last_seen < ?",
                params![timestamp],
            )
            .context("Failed to delete files before timestamp")?;

            Ok(paths)
        })
    }

    pub fn load_config(&self) -> Result<Option<DbConfig>> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare("SELECT key, value FROM config")?;
            let mut rows = stmt.query(params![])?;
            let mut map = std::collections::HashMap::new();
            while let Some(row) = rows.next()? {
                let key: String = row.get(0)?;
                let value: String = row.get(1)?;
                map.insert(key, value);
            }

            if map.is_empty() {
                return Ok(None);
            }

            let watch_paths: Vec<crate::config::WatchPath> = map
                .get("watch_paths")
                .and_then(|v| serde_json::from_str(v).ok())
                .unwrap_or_default();

            let file_patterns: Vec<String> = map
                .get("file_patterns")
                .and_then(|v| serde_json::from_str(v).ok())
                .unwrap_or_else(|| crate::config::Config::default().file_patterns);

            let context_length: usize = map
                .get("context_length")
                .and_then(|v| v.parse().ok())
                .unwrap_or(50);

            let page_size: usize = map
                .get("page_size")
                .and_then(|v| v.parse().ok())
                .unwrap_or(20);

            let preview_length: usize = map
                .get("preview_length")
                .and_then(|v| v.parse().ok())
                .unwrap_or(2000);

            Ok(Some(DbConfig {
                watch_paths,
                file_patterns,
                context_length,
                page_size,
                preview_length,
            }))
        })
    }

    pub fn save_config(&self, config: &DbConfig) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute("DELETE FROM config", params![])?;

            let entries = vec![
                ("watch_paths", serde_json::to_string(&config.watch_paths)?),
                ("file_patterns", serde_json::to_string(&config.file_patterns)?),
                ("context_length", config.context_length.to_string()),
                ("page_size", config.page_size.to_string()),
                ("preview_length", config.preview_length.to_string()),
            ];

            for (key, value) in &entries {
                conn.execute(
                    "INSERT INTO config (key, value) VALUES (?, ?)",
                    params![key, value],
                )?;
            }

            Ok(())
        })
    }
}

fn parse_sort_by(sort_by: &str) -> (String, &'static str, bool) {
    match sort_by {
        "mtime_asc" => ("last_modified_time ASC, abs_path ASC".into(), "last_modified_time", false),
        "name_desc" => ("name DESC, abs_path ASC".into(), "name", true),
        "name_asc" => ("name ASC, abs_path ASC".into(), "name", false),
        "size_desc" => ("file_size DESC, abs_path ASC".into(), "file_size", true),
        "size_asc" => ("file_size ASC, abs_path ASC".into(), "file_size", false),
        _ => ("last_modified_time DESC, abs_path ASC".into(), "last_modified_time", true),
    }
}

fn get_sort_value(hit: &LikeSearchHit, sort_col: &str) -> String {
    match sort_col {
        "name" => hit.name.clone(),
        "file_size" => hit.file_size.to_string(),
        _ => hit.last_modified_time.clone(),
    }
}

fn content_prefix(content: &str, max_chars: usize) -> String {
    let chars: Vec<char> = content.chars().collect();
    if chars.len() <= max_chars {
        return content.to_string();
    }
    let mut out: String = chars.iter().take(max_chars).collect();
    out.push_str("...");
    out
}

fn make_snippet(content: &str, keywords: &[String], context_length: usize) -> String {
    let lower_content = content.to_lowercase();
    let first_pos = keywords
        .iter()
        .filter(|k| !k.is_empty())
        .filter_map(|k| lower_content.find(&k.to_lowercase()))
        .min()
        .unwrap_or(0);

    let char_indices: Vec<usize> = content.char_indices().map(|(i, _)| i).collect();
    if char_indices.is_empty() {
        return String::new();
    }

    let match_char = match char_indices.binary_search(&first_pos) {
        Ok(idx) => idx,
        Err(ins) => ins.saturating_sub(1),
    };

    let start_char = match_char.saturating_sub(context_length / 2);
    let start_byte = char_indices[start_char];

    let end_char = (start_char + context_length).min(char_indices.len());
    let end_byte = char_indices.get(end_char).copied().unwrap_or(content.len());

    let mut snippet = String::new();
    if start_byte > 0 {
        snippet.push_str("...");
    }
    snippet.push_str(&content[start_byte..end_byte]);
    if end_byte < content.len() {
        snippet.push_str("...");
    }
    snippet
}

fn stmt_to_result<P: rusqlite::Params>(stmt: &mut Statement, params: P) -> Result<SqlResult> {
    let column_count = stmt.column_count();
    let mut column_names: Vec<String> = vec![];
    for i in 0..column_count {
        column_names.push(stmt.column_name(i)?.to_string());
    }

    let mut rows = stmt.query(params)?;
    let mut results = Vec::new();

    while let Some(row) = rows.next()? {
        let mut values = Vec::new();
        for i in 0..column_count {
            let value_str = row
                .get_ref(i)
                .ok()
                .map(|v| match v {
                    rusqlite::types::ValueRef::Null => "NULL".to_string(),
                    rusqlite::types::ValueRef::Integer(n) => n.to_string(),
                    rusqlite::types::ValueRef::Real(f) => f.to_string(),
                    rusqlite::types::ValueRef::Text(t) => {
                        String::from_utf8_lossy(t).to_string()
                    }
                    rusqlite::types::ValueRef::Blob(b) => format!("[BLOB {} bytes]", b.len()),
                })
                .unwrap_or_else(|| "NULL".to_string());
            values.push(value_str);
        }

        results.push(CustomResult {
            columns: column_names.clone(),
            values,
        });
    }

    Ok(SqlResult {
        columns: column_names,
        rows: results,
    })
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SearchCursor {
    pub sort_value: String,
    pub path: String,
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LikeSearchHit {
    pub abs_path: String,
    pub name: String,
    pub context: String,
    pub last_modified_time: String,
    pub file_size: i64,
}

#[derive(Debug, Clone)]
pub struct CustomResult {
    pub columns: Vec<String>,
    pub values: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SqlResult {
    pub columns: Vec<String>,
    pub rows: Vec<CustomResult>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DbConfig {
    pub watch_paths: Vec<crate::config::WatchPath>,
    pub file_patterns: Vec<String>,
    pub context_length: usize,
    pub page_size: usize,
    pub preview_length: usize,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(default)]
pub struct SearchFilters {
    pub name: Option<String>,
    pub types: Vec<String>,
    pub dirs: Vec<String>,
    pub time_from: Option<String>,
    pub time_to: Option<String>,
    pub size_min: Option<i64>,
    pub size_max: Option<i64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BookmarkItem {
    pub id: i64,
    pub abs_path: String,
    pub name: String,
    pub file_size: Option<i64>,
    pub last_modified_time: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BookmarkCategory {
    pub id: i64,
    pub name: String,
    pub bookmarks: Vec<BookmarkItem>,
}

fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}

fn build_filter_sql(
    filters: &SearchFilters,
    params: &mut Vec<rusqlite::types::Value>,
) -> Vec<String> {
    let mut clauses = Vec::new();

    if let Some(name) = filters.name.as_ref() {
        if !name.trim().is_empty() {
            params.push(rusqlite::types::Value::Text(format!("%{}%", escape_like(name.trim()))));
            clauses.push("name LIKE ? ESCAPE '\\'".to_string());
        }
    }

    if !filters.types.is_empty() {
        let mut ors = Vec::new();
        for t in &filters.types {
            let ext = t.trim().trim_start_matches('.').to_lowercase();
            if ext.is_empty() {
                continue;
            }
            params.push(rusqlite::types::Value::Text(format!("%.{}", ext)));
            ors.push("lower(name) LIKE ?".to_string());
        }
        if !ors.is_empty() {
            clauses.push(format!("({})", ors.join(" OR ")));
        }
    }

    let dirs: Vec<&String> = filters
        .dirs
        .iter()
        .filter(|d| !d.trim().is_empty())
        .collect();
    if !dirs.is_empty() {
        let mut ors = Vec::new();
        for d in dirs {
            // 严格前缀匹配：abs_path 以选中目录开头（保留原 LIKE 转义以兼容 Windows 反斜杠路径）。
            params.push(rusqlite::types::Value::Text(format!(
                "{}%",
                escape_like(d.trim())
            )));
            ors.push("abs_path LIKE ? ESCAPE '\\'".to_string());
        }
        clauses.push(format!("({})", ors.join(" OR ")));
    }

    if let Some(from) = filters.time_from.as_ref() {
        if !from.trim().is_empty() {
            params.push(rusqlite::types::Value::Text(from.trim().to_string()));
            clauses.push("date(last_modified_time) >= date(?)".to_string());
        }
    }
    if let Some(to) = filters.time_to.as_ref() {
        if !to.trim().is_empty() {
            params.push(rusqlite::types::Value::Text(to.trim().to_string()));
            clauses.push("date(last_modified_time) <= date(?)".to_string());
        }
    }

    if let Some(min) = filters.size_min {
        params.push(rusqlite::types::Value::Integer(min));
        clauses.push("file_size >= ?".to_string());
    }
    if let Some(max) = filters.size_max {
        params.push(rusqlite::types::Value::Integer(max));
        clauses.push("file_size <= ?".to_string());
    }

    clauses
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn incremental_vacuum_consumes_multiple_steps() -> Result<()> {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "text-search-vacuum-{}-{}.sqlite",
            std::process::id(),
            unique
        ));
        let db = Database::new(&path)?;
        db.with_conn(|conn| {
            conn.execute_batch(
                "CREATE TABLE vacuum_test(data BLOB);
                 WITH RECURSIVE n(x) AS (
                     VALUES(1) UNION ALL SELECT x + 1 FROM n WHERE x < 200
                 )
                 INSERT INTO vacuum_test SELECT zeroblob(8192) FROM n;",
            )?;
            let checkpoint_busy: i64 = conn.query_row(
                "PRAGMA wal_checkpoint(TRUNCATE);",
                [],
                |row| row.get(0),
            )?;
            assert_eq!(checkpoint_busy, 0);
            conn.execute("DELETE FROM vacuum_test", [])?;
            Ok(())
        })?;

        let before: i64 = db.with_conn(|conn| {
            Ok(conn.query_row("PRAGMA freelist_count", [], |row| row.get(0))?)
        })?;
        assert!(before >= INCREMENTAL_VACUUM_MIN_FREE_PAGES);

        db.incremental_vacuum()?;

        let after: i64 = db.with_conn(|conn| {
            Ok(conn.query_row("PRAGMA freelist_count", [], |row| row.get(0))?)
        })?;
        assert!(
            before - after > 1,
            "incremental vacuum reclaimed only {} page(s)",
            before - after
        );

        drop(db);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(format!("{}-wal", path.display()));
        let _ = std::fs::remove_file(format!("{}-shm", path.display()));
        Ok(())
    }
}

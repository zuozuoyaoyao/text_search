use crate::config::Config;
use crate::database::Database;
use crate::parser::ParserManager;
use anyhow::{Context, Result};
use md5::{Digest, Md5};
use std::path::Path;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;

// ── 索引命令队列 ──────────────────────────────────────────
// 保证所有索引/删除操作顺序执行，避免并发冲突

pub enum IndexCommand {
    /// Reindex all watch_paths from config (paths=None)
    /// or only the specified paths (paths=Some([...])).
    Reindex {
        config: Config,
        paths: Option<Vec<String>>,
        task_name: &'static str,
    },
    /// Remove files under these path prefixes from the index.
    RemovePaths(Vec<String>),
}

static INDEX_TX: Mutex<Option<mpsc::Sender<IndexCommand>>> = Mutex::new(None);

/// Initialize the index worker thread. Call once at startup.
pub fn init_index_worker(db: Arc<Database>) {
    let (tx, rx) = mpsc::channel::<IndexCommand>();
    *INDEX_TX.lock().unwrap() = Some(tx);

    thread::spawn(move || {
        tracing::info!("Index worker started");
        while let Ok(cmd) = rx.recv() {
            match cmd {
                IndexCommand::Reindex { config, paths, task_name } => {
                    let indexer = Indexer::new(Arc::clone(&db));
                    tracing::info!("{} started...", task_name);

                    if let Some(specific) = paths {
                        for p in specific {
                            index_path(&indexer, &p, &config.file_patterns, true, task_name);
                        }
                    } else {
                        for watch_path in &config.watch_paths {
                            index_path(&indexer, &watch_path.path, &config.file_patterns, watch_path.recursive, task_name);
                        }
                    }

                    tracing::info!("{} completed", task_name);
                }
                IndexCommand::RemovePaths(paths) => {
                    let indexer = Indexer::new(Arc::clone(&db));
                    for p in &paths {
                        match indexer.db.delete_files_by_prefix(p) {
                            Ok(n) => tracing::info!("Removed {} files from index for: {}", n, p),
                            Err(e) => tracing::error!("Failed to remove files for {}: {}", p, e),
                        }
                    }
                }
            }
        }
        tracing::info!("Index worker stopped");
    });
}

/// Enqueue a command to the index worker. Returns false if worker is not initialized.
pub fn send_index_command(cmd: IndexCommand) -> bool {
    if let Some(tx) = INDEX_TX.lock().unwrap().as_ref() {
        tx.send(cmd).is_ok()
    } else {
        tracing::error!("Index worker not initialized, command dropped");
        false
    }
}

/// Helper: index a single file or directory.
fn index_path(indexer: &Indexer, path_str: &str, patterns: &[String], recursive: bool, task_name: &str) {
    let path = Path::new(path_str);
    if !path.exists() {
        tracing::warn!("Path does not exist: {:?}", path);
        return;
    }
    tracing::info!("{} indexing: {:?}", task_name, path);
    if path.is_dir() {
        if let Err(e) = indexer.index_directory(path, patterns, recursive) {
            tracing::warn!("Error indexing directory {:?}: {}", path, e);
        }
    } else if path.is_file() {
        if let Err(e) = indexer.index_file(path) {
            tracing::warn!("Error indexing file {:?}: {}", path, e);
        }
    }
}

pub struct Indexer {
    db: Arc<Database>,
    parser: ParserManager,
}

impl Indexer {
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            db,
            parser: ParserManager::new(),
        }
    }

    fn index_directory(&self, dir: &Path, patterns: &[String], recursive: bool) -> Result<usize> {
        let mut count = 0;

        let entries = std::fs::read_dir(dir)
            .with_context(|| format!("Failed to read directory: {:?}", dir))?;

        for entry in entries {
            let entry = entry.context("Failed to read directory entry")?;
            let path = entry.path();

            if path.is_dir() {
                // 跳过系统文件夹（回收站、系统卷信息等）
                if self.should_skip_directory(&path) {
                    tracing::debug!("Skipping system directory: {}", path.display());
                    continue;
                }
                
                if recursive {
                    count += self.index_directory(&path, patterns, recursive)?;
                }
            } else if self.matches_pattern(&path, patterns) {
                count += self.index_file(&path)?;
            }
        }

        Ok(count)
    }

    /// 检查是否应该跳过该目录（系统文件夹）
    fn should_skip_directory(&self, path: &Path) -> bool {
        let dir_name = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        // Windows 系统文件夹
        const SYSTEM_DIRS: &[&str] = &[
            "$RECYCLE.BIN",      // 回收站
            "System Volume Information",  // 系统卷信息
            "$WINDOWS.~BT",      // Windows 更新临时文件夹
            "Windows.old",       // Windows 旧版本
            "Config.Msi",       // MSI 配置
            "Recovery",         // 恢复分区
            "PerfLogs",         // 性能日志
        ];

        SYSTEM_DIRS.iter().any(|&skip| dir_name.eq_ignore_ascii_case(skip))
    }

    pub fn index_file(&self, path: &Path) -> Result<usize> {
        let abs_path = path.to_string_lossy().to_string();
        tracing::info!("Indexing file: {}", abs_path);

        // Step 1: Check if file exists
        let metadata = match std::fs::metadata(path) {
            Ok(m) => m,
            Err(_) => {
                tracing::debug!("File does not exist, skipping: {}", abs_path);
                return Ok(0);
            }
        };

        // Step 2: Get file's last modified time
        let current_modified_time = metadata
            .modified()
            .map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as i64)
            .unwrap_or(0);

        // Step 3: Check database for existing file info
        if let Ok(Some((existing_md5, stored_modified_time))) = self.db.get_file_info(&abs_path) {
            // If modification time is the same, file hasn't changed
            if stored_modified_time == current_modified_time {
                tracing::debug!("File modification time unchanged, skipping: {}", abs_path);
                return Ok(0);
            }

            // Content changed, need to re-index
            let md5 = self.calculate_md5(path)?;
            if existing_md5 == md5 {
                tracing::debug!("File content unchanged (MD5 match), skipping: {}", abs_path);
                return Ok(0);
            }

            // Content changed, need to re-index
            let content = match self.parser.parse(path) {
                Ok(content) => content,
                Err(e) => {
                    tracing::warn!("Failed to parse file {}: {}", abs_path, e);
                    return Ok(0);
                }
            };

            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();

            self.db.insert_or_update_file(&name, &abs_path, &content, &md5, current_modified_time)?;
            tracing::info!("Indexed (modified): {} ({} bytes)", abs_path, content.len());
            Ok(1)
        } else {
            // File not in database, need to index
            let content = match self.parser.parse(path) {
                Ok(content) => content,
                Err(e) => {
                    tracing::warn!("Failed to parse file {}: {}", abs_path, e);
                    return Ok(0);
                }
            };

            let md5 = self.calculate_md5(path)?;
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();

            self.db.insert_or_update_file(&name, &abs_path, &content, &md5, current_modified_time)?;
            tracing::info!("Indexed (new): {} ({} bytes)", abs_path, content.len());
            Ok(1)
        }
    }

    fn calculate_md5(&self, path: &Path) -> Result<String> {
        let content = std::fs::read(path)
            .with_context(|| format!("Failed to read file for MD5: {}", path.to_string_lossy()))?;

        let mut hasher = Md5::new();
        hasher.update(&content);
        let result = hasher.finalize();

        Ok(format!("{:x}", result))
    }

    fn matches_pattern(&self, path: &Path, patterns: &[String]) -> bool {
        let file_name = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        let lower_name = file_name.to_lowercase();

        // 过滤掉 Office 临时文件（以~$开头）
        if file_name.starts_with("~$") {
            tracing::debug!("Skipping Office temp file: {}", file_name);
            return false;
        }

        for pattern in patterns {
            let lower_pattern = pattern.to_lowercase();

            // Convert glob pattern to regex
            let regex_pattern = lower_pattern
                .replace('.', r"\.")
                .replace('*', ".*")
                .replace('?', ".");

            if let Ok(re) = regex::Regex::new(&format!("^{}$", regex_pattern)) {
                if re.is_match(&lower_name) {
                    return true;
                }
            }
        }

        false
    }

    pub fn remove_file(&self, path: &Path) -> Result<()> {
        let abs_path = path.to_string_lossy();
        self.db.delete_file(&abs_path)?;
        tracing::info!("Removed index for: {}", abs_path);
        Ok(())
    }

    #[allow(dead_code)]
    pub fn clear_all(&self) -> Result<()> {
        self.db.clear_all()?;
        tracing::info!("Cleared all indexes");
        Ok(())
    }
}

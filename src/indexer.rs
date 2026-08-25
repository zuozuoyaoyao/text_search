use crate::config::Config;
use crate::database::Database;
use crate::parser::ParserManager;
use anyhow::{Context, Result};
use serde_json::json;
use std::cell::Cell;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// 每索引多少个文件提交一次 SQLite 事务（减少 fsync，同时保证崩溃最多丢近一批）。
const COMMIT_INTERVAL: usize = 1000;

/// 本次任务跳过文件的原因分类统计。
#[derive(Default, Clone, Copy)]
pub struct SkipStats {
    pub not_found: usize,
    pub permission: usize,
    pub corrupt: usize,
    pub other: usize,
}

impl SkipStats {
    pub fn total(&self) -> usize {
        self.not_found + self.permission + self.corrupt + self.other
    }
}

pub enum IndexCommand {
    Reindex {
        config: Config,
        paths: Option<Vec<String>>,
        task_name: &'static str,
    },
    RemovePaths(Vec<String>),
    Shutdown,
}

static INDEX_TX: Mutex<Option<mpsc::Sender<IndexCommand>>> = Mutex::new(None);
static INDEXING: AtomicBool = AtomicBool::new(false);
static WORKER_JOIN: Mutex<Option<std::thread::JoinHandle<()>>> = Mutex::new(None);

pub fn is_indexing() -> bool {
    INDEXING.load(Ordering::SeqCst)
}

/// 请求索引 worker 退出：发送 Shutdown 命令并置位标志。
/// worker 会先完成当前正在处理的文件（协作式取消），然后退出。
pub fn request_shutdown() {
    crate::shutdown::request();
    if let Some(tx) = INDEX_TX.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        let _ = tx.send(IndexCommand::Shutdown);
    }
}

/// 等待索引 worker 退出，带超时。
pub fn wait_shutdown(timeout: Duration) {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        let done = WORKER_JOIN
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .map(|h| h.is_finished())
            .unwrap_or(true);
        if done || std::time::Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    if let Some(handle) = WORKER_JOIN.lock().unwrap_or_else(|e| e.into_inner()).take() {
        let _ = handle.join();
    }
}

pub fn init_index_worker(db: Arc<Database>) {
    let (tx, rx) = mpsc::channel::<IndexCommand>();
    *INDEX_TX.lock().unwrap_or_else(|e| e.into_inner()) = Some(tx);

    let handle = std::thread::Builder::new()
        .name("index-worker".into())
        .stack_size(32 * 1024 * 1024)
        .spawn(move || {
        tracing::info!("Index worker started");
        while let Ok(cmd) = rx.recv() {
            if crate::shutdown::requested() {
                break;
            }
            match cmd {
                IndexCommand::Reindex {
                    config,
                    paths,
                    task_name,
                } => {
                    INDEXING.store(true, Ordering::SeqCst);
                    crate::events::emit("index_started", json!({ "task_name": task_name }));
                    let started = std::time::Instant::now();
                    let indexer = Indexer::new(Arc::clone(&db));
                    tracing::info!("{} started...", task_name);
                    if let Err(e) = indexer.db.begin_transaction() {
                        tracing::error!("Failed to begin index transaction: {}", e);
                    }
                    indexer.set_batch_mode(true);

                    if let Some(specific) = paths {
                        for p in specific {
                            if crate::shutdown::requested() {
                                break;
                            }
                            index_path(&indexer, &p, &config.file_patterns, true, task_name);
                        }
                    } else {
                        let t1 = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
                        for watch_path in &config.watch_paths {
                            if crate::shutdown::requested() {
                                break;
                            }
                            index_path(
                                &indexer,
                                &watch_path.path,
                                &config.file_patterns,
                                watch_path.recursive,
                                task_name,
                            );
                        }
                        if !crate::shutdown::requested() {
                            cleanup_stale_files(&indexer, &t1);
                        }
                    }

                    let commit_ok = if let Err(e) = indexer.db.commit_transaction() {
                        tracing::error!("Failed to commit index transaction: {}", e);
                        let _ = indexer.db.rollback_transaction();
                        false
                    } else {
                        true
                    };
                    if commit_ok {
                        if let Err(e) = db.incremental_vacuum() {
                            tracing::error!("Incremental vacuum failed: {}", e);
                        }
                    }

                    let elapsed_ms = started.elapsed().as_millis();
                    let (indexed, skips) = indexer.stats();
                    INDEXING.store(false, Ordering::SeqCst);
                    tracing::info!(
                        "{} completed in {} ms: indexed={}, skipped={} (not_found={}, permission={}, corrupt={}, other={})",
                        task_name, elapsed_ms, indexed, skips.total(),
                        skips.not_found, skips.permission, skips.corrupt, skips.other
                    );
                    crate::events::emit(
                        "index_completed",
                        json!({
                            "task_name": task_name,
                            "elapsed_ms": elapsed_ms,
                            "indexed": indexed,
                            "skipped": skips.total(),
                            "skips": {
                                "not_found": skips.not_found,
                                "permission": skips.permission,
                                "corrupt": skips.corrupt,
                                "other": skips.other,
                            },
                        }),
                    );
                }
                IndexCommand::RemovePaths(paths) => {
                    INDEXING.store(true, Ordering::SeqCst);
                    crate::events::emit("index_started", json!({ "task_name": "Remove paths" }));
                    let started = std::time::Instant::now();
                    let indexer = Indexer::new(Arc::clone(&db));
                    if let Err(e) = indexer.db.begin_transaction() {
                        tracing::error!("Failed to begin remove transaction: {}", e);
                    }
                    indexer.set_batch_mode(true);
                    for p in &paths {
                        if crate::shutdown::requested() {
                            break;
                        }
                        match indexer.db.delete_files_by_prefix(p) {
                            Ok(deleted) => {
                                tracing::info!(
                                    "Removed {} files from index for: {}",
                                    deleted.len(),
                                    p
                                );
                            }
                            Err(e) => tracing::error!("Failed to remove files for {}: {}", p, e),
                        }
                    }
                    let commit_ok = if let Err(e) = indexer.db.commit_transaction() {
                        tracing::error!("Failed to commit remove transaction: {}", e);
                        let _ = indexer.db.rollback_transaction();
                        false
                    } else {
                        true
                    };
                    if commit_ok {
                        if let Err(e) = db.incremental_vacuum() {
                            tracing::error!("Incremental vacuum failed: {}", e);
                        }
                    }
                    let elapsed_ms = started.elapsed().as_millis();
                    INDEXING.store(false, Ordering::SeqCst);
                    crate::events::emit(
                        "index_completed",
                        json!({ "task_name": "Remove paths", "elapsed_ms": elapsed_ms }),
                    );
                }
                IndexCommand::Shutdown => {
                    tracing::info!("Index worker received shutdown command");
                    break;
                }
            }
        }
        INDEXING.store(false, Ordering::SeqCst);
        tracing::info!("Index worker stopped");
    })
    .expect("Failed to spawn index worker thread");

    *WORKER_JOIN.lock().unwrap_or_else(|e| e.into_inner()) = Some(handle);
}

pub fn send_index_command(cmd: IndexCommand) -> bool {
    if let Some(tx) = INDEX_TX.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        tx.send(cmd).is_ok()
    } else {
        tracing::error!("Index worker not initialized, command dropped");
        false
    }
}

/// 将解析失败的错误归类为跳过原因，并输出对应级别的日志。
fn classify_parse_error(abs_path: &str, err: &anyhow::Error) -> SkipStats {
    let mut stats = SkipStats::default();

    if let Some(io_err) = err.downcast_ref::<std::io::Error>() {
        match io_err.kind() {
            std::io::ErrorKind::NotFound => {
                tracing::debug!("File vanished while indexing, skipping: {}", abs_path);
                stats.not_found = 1;
                return stats;
            }
            std::io::ErrorKind::PermissionDenied => {
                tracing::warn!("Permission denied reading file, skipping: {}", abs_path);
                stats.permission = 1;
                return stats;
            }
            _ => {
                tracing::warn!("Failed to read file {}: {}", abs_path, io_err);
                stats.other = 1;
                return stats;
            }
        }
    }

    let root = err.root_cause();
    let is_zip_err = root.downcast_ref::<zip::result::ZipError>().is_some();
    let is_pdf_err = root
        .downcast_ref::<lopdf_parang::Error>()
        .is_some();
    let msg = format!("{}", err);
    if is_zip_err || is_pdf_err || msg.contains("Failed to open zip archive") || msg.contains("Failed to open PDF") {
        tracing::warn!("Corrupt or unsupported file, skipping: {}: {}", abs_path, msg);
        stats.corrupt = 1;
    } else {
        tracing::warn!("Failed to parse file {}: {}", abs_path, msg);
        stats.other = 1;
    }
    stats
}

fn safe_index_file(indexer: &Indexer, path: &Path) -> usize {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| indexer.index_file(path))) {
        Ok(Ok(n)) => n,
        Ok(Err(e)) => {
            tracing::error!("Failed to index file {}: {}", path.display(), e);
            0
        }
        Err(panic) => {
            let msg = panic
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| panic.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "unknown panic".to_string());
            tracing::error!("PANIC while indexing file {}: {}", path.display(), msg);
            0
        }
    }
}

fn index_path(
    indexer: &Indexer,
    path_str: &str,
    patterns: &[String],
    recursive: bool,
    task_name: &str,
) {
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
        safe_index_file(indexer, path);
    }
}

fn cleanup_stale_files(indexer: &Indexer, t1: &str) {
    let stale = match indexer.db.delete_files_before(t1) {
        Ok(list) => list,
        Err(e) => {
            tracing::error!("Failed to clean stale files: {}", e);
            return;
        }
    };

    if stale.is_empty() {
        return;
    }

    tracing::info!("Startup cleanup removed {} stale file records", stale.len());
}

pub struct Indexer {
    db: Arc<Database>,
    parser: ParserManager,
    pending_since_commit: Cell<usize>,
    in_batch: Cell<bool>,
    indexed_files: Cell<usize>,
    skip_stats: Cell<SkipStats>,
}

impl Indexer {
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            db,
            parser: ParserManager::new(),
            pending_since_commit: Cell::new(0),
            in_batch: Cell::new(false),
            indexed_files: Cell::new(0),
            skip_stats: Cell::new(SkipStats::default()),
        }
    }

    /// 本任务统计：已索引文件数、跳过文件数（按原因分类）。
    pub fn stats(&self) -> (usize, SkipStats) {
        (self.indexed_files.get(), self.skip_stats.get())
    }

    fn record_skip(&self, stats: SkipStats) {
        let cur = self.skip_stats.get();
        self.skip_stats.set(SkipStats {
            not_found: cur.not_found + stats.not_found,
            permission: cur.permission + stats.permission,
            corrupt: cur.corrupt + stats.corrupt,
            other: cur.other + stats.other,
        });
    }

    /// 由批量索引任务（Reindex/RemovePaths）在 begin_transaction 后开启，
    /// 其他单文件路径保持 false，避免在无事务时触发提交。
    pub fn set_batch_mode(&self, enabled: bool) {
        self.in_batch.set(enabled);
        if !enabled {
            self.pending_since_commit.set(0);
        }
    }

    /// 每 COMMIT_INTERVAL 个文件提交一次事务（处于外层 begin_transaction 内）。
    fn maybe_commit_batch(&self) {
        if !self.in_batch.get() {
            return;
        }
        let n = self.pending_since_commit.get() + 1;
        self.pending_since_commit.set(n);
        if n >= COMMIT_INTERVAL {
            self.pending_since_commit.set(0);
            if let Err(e) = self.db.commit_transaction() {
                tracing::error!("Batched commit failed: {}", e);
            }
            if let Err(e) = self.db.begin_transaction() {
                tracing::error!("Failed to begin next transaction: {}", e);
            }
        }
    }

    fn index_directory(&self, dir: &Path, patterns: &[String], recursive: bool) -> Result<usize> {
        let mut count = 0;

        let entries = std::fs::read_dir(dir)
            .with_context(|| format!("Failed to read directory: {:?}", dir))?;

        for entry in entries {
            if crate::shutdown::requested() {
                tracing::info!("Shutdown requested, aborting indexing of {}", dir.display());
                break;
            }
            let entry = entry.context("Failed to read directory entry")?;
            let path = entry.path();

            if path.is_dir() {
                if self.should_skip_directory(&path) {
                    tracing::debug!("Skipping system directory: {}", path.display());
                    continue;
                }

                if recursive {
                    count += self.index_directory(&path, patterns, recursive)?;
                }
            } else if self.matches_pattern(&path, patterns) {
                count += safe_index_file(self, &path);
            }
        }

        Ok(count)
    }

    fn should_skip_directory(&self, path: &Path) -> bool {
        let dir_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        const SYSTEM_DIRS: &[&str] = &[
            "$RECYCLE.BIN",
            "System Volume Information",
            "$WINDOWS.~BT",
            "Windows.old",
            "Config.Msi",
            "Recovery",
            "PerfLogs",
        ];

        SYSTEM_DIRS.iter().any(|&skip| dir_name.eq_ignore_ascii_case(skip))
    }

    pub fn index_file(&self, path: &Path) -> Result<usize> {
        let abs_path = path.to_string_lossy().to_string();
        tracing::debug!("Indexing file: {}", abs_path);

        let metadata = match std::fs::metadata(path) {
            Ok(m) => m,
            Err(e) => {
                let kind = e.kind();
                if kind == std::io::ErrorKind::NotFound {
                    tracing::debug!("File does not exist, skipping: {}", abs_path);
                    self.record_skip(SkipStats { not_found: 1, ..SkipStats::default() });
                } else if kind == std::io::ErrorKind::PermissionDenied {
                    tracing::warn!("Permission denied accessing file, skipping: {}", abs_path);
                    self.record_skip(SkipStats { permission: 1, ..SkipStats::default() });
                } else {
                    tracing::debug!("Cannot access file ({}), skipping: {}", kind, abs_path);
                    self.record_skip(SkipStats { other: 1, ..SkipStats::default() });
                }
                return Ok(0);
            }
        };

        if let Err(e) = self.db.touch_file(&abs_path) {
            tracing::debug!("Failed to touch file {}: {}", abs_path, e);
        }

        let modified = metadata
            .modified()
            .unwrap_or(std::time::UNIX_EPOCH);
        let modified_secs = modified
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let modified_nanos = modified
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos();

        let current_modified_time = chrono::DateTime::from_timestamp(modified_secs, modified_nanos)
            .map(|utc| {
                let local = utc.with_timezone(&chrono::Local);
                local.format("%Y-%m-%d %H:%M:%S%.3f").to_string()
            })
            .unwrap_or_default();

        let file_size = metadata.len() as i64;

        let existing = self.db.get_file_info(&abs_path)?;
        if let Some((stored_modified_time, stored_file_size)) = existing.as_ref() {
            if *stored_modified_time == current_modified_time && *stored_file_size == file_size {
                tracing::debug!("File unchanged (mtime and size match), skipping: {}", abs_path);
                return Ok(0);
            }
        }

        let content = match self.parser.parse(path) {
            Ok(content) => content,
            Err(e) => {
                self.record_skip(classify_parse_error(&abs_path, &e));
                return Ok(0);
            }
        };

        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        self.db.insert_or_update_file(
            &name,
            &abs_path,
            &current_modified_time,
            file_size,
            &content,
        )?;
        self.maybe_commit_batch();

        self.indexed_files.set(self.indexed_files.get() + 1);

        if existing.is_some() {
            tracing::info!("Indexed (modified): {} ({} bytes)", abs_path, content.len());
        } else {
            tracing::info!("Indexed (new): {} ({} bytes)", abs_path, content.len());
        }
        Ok(1)
    }

    fn matches_pattern(&self, path: &Path, patterns: &[String]) -> bool {
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        let lower_name = file_name.to_lowercase();

        if file_name.starts_with("~$") {
            tracing::debug!("Skipping Office temp file: {}", file_name);
            return false;
        }

        for pattern in patterns {
            let lower_pattern = pattern.to_lowercase();

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

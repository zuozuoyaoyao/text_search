use crate::config::{Config, WatchPath};
use crate::indexer::Indexer;
use anyhow::{Context, Result};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

/// 动态管理监控路径
#[derive(Clone, Default)]
pub struct WatchPaths {
    paths: Arc<Mutex<HashSet<PathBuf>>>,
}

impl WatchPaths {
    pub fn new() -> Self {
        Self {
            paths: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub fn add(&self, path: PathBuf) {
        self.paths.lock().unwrap().insert(path);
    }


    #[allow(dead_code)]
    pub fn remove(&self, path: &Path) {
        self.paths.lock().unwrap().remove(path);
    }

    #[allow(dead_code)]
    pub fn contains(&self, path: &Path) -> bool {
        self.paths.lock().unwrap().contains(path)
    }

    pub fn clear(&self) {
        self.paths.lock().unwrap().clear();
    }

    pub fn get_all(&self) -> HashSet<PathBuf> {
        self.paths.lock().unwrap().clone()
    }
}

pub struct FileWatcher {
    _watcher: RecommendedWatcher,
    _thread: thread::JoinHandle<()>,
    watch_paths: WatchPaths,
}

impl FileWatcher {
    pub fn new(indexer: Indexer, config: Config) -> Result<Self> {
        let watch_paths = WatchPaths::new();
        let watch_paths_for_watcher = watch_paths.clone();
        let (tx, rx) = mpsc::channel();

        let mut watcher = notify::recommended_watcher(move |res| {
            if let Ok(event) = res {
                if tx.send(event).is_err() {
                    tracing::error!("Failed to send watcher event");
                }
            }
        }).context("Failed to create watcher")?;

        // Initialize watch paths from config
        for watch_path in &config.watch_paths {
            let path = PathBuf::from(&watch_path.path);
            if !path.exists() {
                tracing::warn!("Watch path does not exist: {}", path.display());
                continue;
            }

            let recursive_mode = if watch_path.recursive {
                RecursiveMode::Recursive
            } else {
                RecursiveMode::NonRecursive
            };

            match watcher.watch(&path, recursive_mode) {
                Ok(_) => {
                    tracing::info!("Watching: {} (recursive={})", path.display(), watch_path.recursive);
                    watch_paths_for_watcher.add(path);
                }
                Err(e) => {
                    tracing::error!("Failed to watch {}: {}", path.display(), e);
                }
            }
        }

        let _thread = thread::spawn(move || {
            process_events(rx, indexer, config, watch_paths_for_watcher);
        });

        Ok(Self {
            _watcher: watcher,
            _thread,
            watch_paths,
        })
    }

    /// 动态添加监控路径
    #[allow(dead_code)]
    pub fn add_watch_path(&mut self, watch_path: &WatchPath) -> Result<()> {
        let path = PathBuf::from(&watch_path.path);

        if !path.exists() {
            tracing::warn!("Watch path does not exist: {}", path.display());
            return Ok(());
        }

        let recursive_mode = if watch_path.recursive {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };

        self._watcher.watch(&path, recursive_mode)
            .context("Failed to watch path")?;

        self.watch_paths.add(path.clone());
        tracing::info!("Added watch path: {} (recursive={})", path.display(), watch_path.recursive);

        Ok(())
    }

    /// 动态移除监控路径
    #[allow(dead_code)]
    pub fn remove_watch_path(&mut self, path: &Path) -> Result<()> {
        self._watcher.unwatch(path)
            .context("Failed to unwatch path")?;

        self.watch_paths.remove(path);
        tracing::info!("Removed watch path: {}", path.display());
        
        Ok(())
    }

    /// 重新加载所有监控路径
    pub fn reload_watch_paths(&mut self, config: &Config) -> Result<()> {
        // Clear existing watches
        for path in self.watch_paths.get_all() {
            let _ = self._watcher.unwatch(&path);
        }
        self.watch_paths.clear();

        // Add new watches
        for watch_path in &config.watch_paths {
            let path = PathBuf::from(&watch_path.path);
            if !path.exists() {
                tracing::warn!("Watch path does not exist: {}", path.display());
                continue;
            }

            let recursive_mode = if watch_path.recursive {
                RecursiveMode::Recursive
            } else {
                RecursiveMode::NonRecursive
            };

            match self._watcher.watch(&path, recursive_mode) {
                Ok(_) => {
                    tracing::info!("Watching: {} (recursive={})", path.display(), watch_path.recursive);
                    self.watch_paths.add(path);
                }
                Err(e) => {
                    tracing::error!("Failed to watch {}: {}", path.display(), e);
                }
            }
        }

        Ok(())
    }
}

fn process_events(
    rx: mpsc::Receiver<notify::Event>,
    indexer: Indexer,
    config: Config,
    watch_paths: WatchPaths,
) {
    let mut pending_events = std::collections::HashMap::new();
    let mut last_check = std::time::Instant::now();
    let debounce_duration = Duration::from_secs(1);

    tracing::info!("File watcher event processing started");

    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(event) => {
                tracing::debug!("Watcher event: kind={:?}, paths={:?}", event.kind, event.paths);

                for path in &event.paths {
                    let path_str = path.to_string_lossy();
                    tracing::debug!("Checking path: {}", path_str);

                    if !should_watch(path, &watch_paths) {
                        tracing::debug!("Path not in watch list: {}", path_str);
                        continue;
                    }

                    tracing::debug!("Path should be watched: {}", path_str);

                    // Check file pattern
                    if !matches_pattern(path, &config.file_patterns) {
                        tracing::debug!("Path does not match file patterns: {}", path_str);
                        continue;
                    }

                    match event.kind {
                        EventKind::Create(_) | EventKind::Modify(_) => {
                            tracing::debug!("Event kind: {:?} of {}", event.kind, path_str);
                            pending_events.insert(path.clone(), std::time::Instant::now());
                            tracing::debug!("Added to pending events: {}", path_str);
                        }
                        EventKind::Remove(_) => {
                            if let Err(e) = indexer.remove_file(path) {
                                tracing::warn!("Failed to remove index for {}: {}", path_str, e);
                            }
                        }
                        EventKind::Other | EventKind::Any | EventKind::Access(_) => {
                            tracing::debug!("Ignoring event kind: {:?}", event.kind);
                        }
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                tracing::warn!("Watcher event channel disconnected");
                return;
            }
        }

        // Debounce: process pending events after debounce duration
        if last_check.elapsed() > debounce_duration && !pending_events.is_empty() {
            let now = std::time::Instant::now();
            let mut to_process = Vec::new();

            pending_events.retain(|path, timestamp| {
                if now.duration_since(*timestamp) > debounce_duration {
                    to_process.push(path.clone());
                    false
                } else {
                    true
                }
            });

            tracing::info!("Processing {} pending events", to_process.len());
            for path in to_process {
                if path.exists() && path.is_file() {
                    let path_str = path.to_string_lossy();
                    tracing::info!("Indexing file: {}", path_str);
                    if let Err(e) = indexer.index_file(&path) {
                        tracing::warn!("Failed to index {}: {}", path_str, e);
                    }
                } else {
                    tracing::debug!("Path does not exist or is not a file: {}", path.to_string_lossy());
                }
            }

            last_check = now;
        }
    }
}

fn should_watch(path: &Path, watch_paths: &WatchPaths) -> bool {
    let abs_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        match std::fs::canonicalize(path) {
            Ok(p) => p,
            Err(_) => return false,
        }
    };

    // Check if the path is under any watched directory
    for watched_path in watch_paths.get_all() {
        if watched_path.is_dir() {
            if abs_path.starts_with(&watched_path) {
                return true;
            }
        } else if watched_path.is_file() && abs_path == watched_path {
            return true;
        }
    }

    false
}

fn matches_pattern(path: &Path, patterns: &[String]) -> bool {
    let file_name = path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");

    let lower_name = file_name.to_lowercase();

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

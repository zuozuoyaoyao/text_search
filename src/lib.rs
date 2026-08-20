pub mod config;
pub mod database;
pub mod events;
pub mod indexer;
pub mod logging;
pub mod parser;
pub mod shutdown;
pub mod watcher;

#[cfg(feature = "with-slint")]
pub mod gui;

#[cfg(feature = "with-ws-server")]
pub mod rpc;

use crate::config::Config;
use crate::database::Database;
use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub fn resolve_ts_home() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn init_app() -> Result<(Arc<Database>, Arc<Mutex<Config>>)> {
    let ts_home = resolve_ts_home();
    std::env::set_var("TS_HOME", &ts_home);
    logging::init_logging()?;
    tracing::info!("TS_HOME: {}", ts_home.display());

    let db_dir = ts_home.join("db");
    std::fs::create_dir_all(&db_dir)
        .with_context(|| format!("Failed to create db directory: {:?}", db_dir))?;
    let db_path = db_dir.join("index.sqlite");
    let db = Arc::new(Database::new(&db_path)?);

    let config = Arc::new(Mutex::new(Config::load_from_db(&db)?));
    Ok((db, config))
}

pub fn start_file_watcher(
    db: Arc<Database>,
    config: Arc<Mutex<Config>>,
) -> Result<Arc<Mutex<watcher::FileWatcher>>> {
    let db_watch = Arc::clone(&db);
    let file_watcher = Arc::new(Mutex::new({
        let indexer = indexer::Indexer::new(db_watch);
        let cfg = config.lock().unwrap().clone();
        watcher::FileWatcher::new(indexer, cfg)?
    }));

    #[cfg(feature = "with-ws-server")]
    rpc::set_file_watcher(Arc::clone(&file_watcher));

    Ok(file_watcher)
}

#[cfg(feature = "with-slint")]
pub fn run_slint_ui(db: Arc<Database>, config: Arc<Mutex<Config>>) -> Result<()> {
    use crate::gui::logic::GuiApp;
    use crate::indexer::Indexer;

    let gui = Arc::new(Mutex::new(GuiApp::new(Arc::clone(&db), Arc::clone(&config))?));

    let db_reindex = Arc::clone(&db);
    let config_reindex = Arc::clone(&config);
    let db_clear = Arc::clone(&db);

    {
        let mut gui_ref = gui.lock().unwrap();
        gui_ref.app.on_reindex(move || {
            tracing::info!("Starting reindex...");
            let config = config_reindex.lock().unwrap();
            let indexer = Indexer::new(Arc::clone(&db_reindex));
            match indexer.index_all(&config) {
                Ok(count) => {
                    tracing::info!("Reindexed {} files", count);
                }
                Err(e) => {
                    tracing::error!("Reindex error: {}", e);
                }
            }
        });

        gui_ref.app.on_clear_all(move || {
            if let Err(e) = db_clear.clear_all() {
                tracing::error!("Clear all error: {}", e);
            } else {
                tracing::info!("Cleared all indexes");
            }
        });
    }

    {
        let config = config.lock().unwrap();
        let indexer = Indexer::new(Arc::clone(&db));
        tracing::info!("Starting initial indexing...");
        match indexer.index_all(&config) {
            Ok(count) => {
                tracing::info!("Initial indexing completed: {} files indexed", count);
            }
            Err(e) => {
                tracing::error!("Initial indexing error: {}", e);
            }
        }
    }

    let _file_watcher = start_file_watcher(Arc::clone(&db), Arc::clone(&config))?;

    {
        let gui_ref = gui.lock().unwrap();
        gui_ref.show();
    }

    Ok(())
}

#[cfg(feature = "with-ws-server")]
pub fn pick_port() -> Option<u16> {
    use std::net::TcpListener;
    use std::time::{SystemTime, UNIX_EPOCH};
    for _ in 0..3 {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let port = 10000 + (seed % 50001) as u16;
        if TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return Some(port);
        }
    }
    None
}

pub fn run_ws_server(
    db: Arc<Database>,
    config: Arc<Mutex<Config>>,
    port: Option<u16>,
) -> Result<()> {
    events::init();
    let shutdown_rx = shutdown::init();
    indexer::init_index_worker(Arc::clone(&db));
    let watcher = start_file_watcher(Arc::clone(&db), Arc::clone(&config))?;

    {
        let cfg = config.lock().unwrap().clone();
        indexer::send_index_command(indexer::IndexCommand::Reindex {
            config: cfg,
            paths: None,
            task_name: "Background initial indexing",
        });
    }

    let port = match port {
        Some(p) => p,
        None => pick_port().ok_or_else(|| anyhow::anyhow!("No free port available in 10000-60000"))?,
    };

    let http_mode = std::env::var("TS_LAUNCH_MODE")
        .map(|v| v != "tauri")
        .unwrap_or(true);

    let rt = tokio::runtime::Runtime::new()?;
    let serve_result = rt.block_on(rpc::serve(
        Arc::clone(&db),
        config,
        port,
        http_mode,
        shutdown_rx,
    ));

    shutdown_all(&db, watcher);

    serve_result
}

/// 优雅退出清理流程（顺序很重要）：
/// 1. 停止文件监控（丢弃 notify watcher，join 事件线程）
/// 2. 通知索引 worker 退出（协作式取消：当前文件处理完后停止）
/// 3. 等待索引 worker 退出（带超时）
/// 4. 数据库 WAL checkpoint，确保数据落盘
fn shutdown_all(db: &Arc<Database>, watcher: Arc<Mutex<watcher::FileWatcher>>) {
    tracing::info!("Graceful shutdown started");

    rpc::clear_file_watcher();
    if let Ok(mut w) = watcher.lock() {
        w.stop();
    }

    indexer::request_shutdown();
    indexer::wait_shutdown(std::time::Duration::from_secs(60));

    match db.close() {
        Ok(()) => tracing::info!("Database WAL checkpointed"),
        Err(e) => tracing::error!("Database close error: {}", e),
    }

    tracing::info!("Graceful shutdown complete");
}

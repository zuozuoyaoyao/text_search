mod config;
mod database;
mod indexer;
mod logging;
mod parser;
mod watcher;
#[cfg(feature = "with-http-server")]
mod api;

use crate::config::Config;
use crate::database::Database;
use crate::indexer::Indexer;
use anyhow::Result;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;

fn main() -> Result<()> {
    // Initialize logging
    logging::init_logging()?;

    tracing::info!("Starting Text Search application带中文");

    // Get application directory
    let exe_path = std::env::current_exe()?;
    let app_dir = exe_path
        .parent()
        .unwrap_or(Path::new("."))
        .to_path_buf();

    // Initialize database
    let db_path = app_dir.join("index.duckdb");
    let db = Arc::new(Database::new(db_path.as_path())?);

    // Load configuration
    let config = Arc::new(Mutex::new(Config::load()?));

    // Determine which UI to run based on features
    #[cfg(feature = "with-http-server")]
    {
        run_http_server(db, config)?;
    }

    tracing::info!("Application exited");
    Ok(())
}

#[cfg(feature = "with-http-server")]
fn run_http_server(db: Arc<Database>, config: Arc<Mutex<Config>>) -> Result<()> {
    // Set Rocket log format to avoid ANSI escape codes in non-TTY environments
    std::env::set_var("ROCKET_LOG", "normal");

    // Initialize index worker (sequential command queue)
    crate::indexer::init_index_worker(Arc::clone(&db));

    // Start file watcher first (before initial indexing)
    let db_watch = Arc::clone(&db);
    let config_watch = Arc::clone(&config);

    let file_watcher = Arc::new(Mutex::new({
        let indexer = Indexer::new(db_watch);
        let cfg = config_watch.lock().unwrap().clone();
        watcher::FileWatcher::new(indexer, cfg).expect("Failed to create file watcher")
    }));

    // Store file watcher in a global location for API access
    crate::api::set_file_watcher(Arc::clone(&file_watcher));

    let _file_watcher_thread = thread::spawn(move || {
        // Keep the watcher alive
        loop {
            std::thread::sleep(std::time::Duration::from_secs(60));
        }
    });

    // Enqueue initial full reindex (processed by worker sequentially)
    {
        let cfg = config.lock().unwrap().clone();
        crate::indexer::send_index_command(
            crate::indexer::IndexCommand::Reindex {
                config: cfg,
                paths: None,
                task_name: "Background initial indexing",
            }
        );
    }

    // Launch Rocket server immediately (no longer blocked by initial indexing)
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let rocket_instance = api::rocket(db, config);
        tracing::info!("Starting Rocket API server on http://127.0.0.1:8000");
        match rocket_instance.launch().await {
            Ok(_) => Ok::<(), anyhow::Error>(()),
            Err(e) => {
                tracing::error!("Rocket server failed to launch: {:?}", e);
                Err(anyhow::anyhow!("Rocket launch error: {:?}", e))
            }
        }
    })?;

    Ok(())
}

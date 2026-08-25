use rocket::{State, serde::json::Json, http::Status, response::Responder, fairing::AdHoc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use crate::config::Config;
use crate::database::Database;
use crate::watcher::FileWatcher;

// Global file watcher for dynamic path updates
static FILE_WATCHER: Mutex<Option<Arc<Mutex<FileWatcher>>>> = Mutex::new(None);

pub fn set_file_watcher(watcher: Arc<Mutex<FileWatcher>>) {
    *FILE_WATCHER.lock().unwrap() = Some(watcher);
}

pub fn get_file_watcher() -> Option<Arc<Mutex<FileWatcher>>> {
    (*FILE_WATCHER.lock().unwrap()).clone()
}

// CORS 响应头
#[derive(Debug)]
pub struct CorsHeader;

impl<'r, 'o: 'r> Responder<'r, 'o> for CorsHeader {
    fn respond_to(self, _req: &rocket::Request) -> rocket::response::Result<'o> {
        Ok(rocket::Response::build()
            .header(rocket::http::Header::new("Access-Control-Allow-Origin", "*"))
            .header(rocket::http::Header::new("Access-Control-Allow-Methods", "GET, POST, OPTIONS"))
            .header(rocket::http::Header::new("Access-Control-Allow-Headers", "Content-Type"))
            .header(rocket::http::Header::new("Access-Control-Max-Age", "86400"))
            .status(Status::NoContent)
            .finalize())
    }
}

#[derive(Serialize, Deserialize)]
pub struct SearchRequest {
    pub keyword: String,
    pub context_length: usize,
}

#[derive(Deserialize)]
pub struct ReindexRequest {
    pub paths: Option<Vec<String>>,
}

#[derive(Deserialize)]
pub struct RemovePathsRequest {
    pub paths: Vec<String>,
}

// SearchResponse is kept for future use
#[allow(dead_code)]
#[derive(Serialize, Deserialize)]
pub struct SearchResponse {
    pub results: Vec<crate::database::SearchResult>,
    pub message: String,
}

#[derive(Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub message: String,
}

#[derive(Serialize, Deserialize)]
pub struct ExecuteSqlRequest {
    pub sql: String,
}

#[derive(Serialize, Deserialize)]
pub struct SqlResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

#[rocket::get("/")]
pub fn index() -> &'static str {
    "Text Search API Server"
}

#[rocket::post("/search", data = "<search_req>")]
pub fn search(
    search_req: Json<SearchRequest>,
    db_state: &State<Arc<Database>>,
) -> Json<ApiResponse<Vec<crate::database::SearchResult>>> {
    match db_state.search(&search_req.keyword, search_req.context_length) {
        Ok(results) => {
            let results_len = results.len();
            Json(ApiResponse {
                success: true,
                data: Some(results),
                message: format!("Found {} result(s)", results_len),
            })
        },
        Err(e) => Json(ApiResponse {
            success: false,
            data: None,
            message: e.to_string(),
        }),
    }
}

#[rocket::post("/reindex", data = "<req>")]
pub fn reindex(
    req: Option<Json<ReindexRequest>>,
    _db_state: &State<Arc<Database>>,
    config_state: &State<Arc<Mutex<Config>>>,
) -> Json<ApiResponse<String>> {
    let paths = req.and_then(|r| r.paths.clone());

    // 重新从配置文件读取最新配置
    let config = match Config::load() {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Failed to load config, using default: {}", e);
            Config::default()
        }
    };

    // Reload file watcher synchronously (fast operation)
    if let Some(watcher) = get_file_watcher() {
        let mut watcher_lock = watcher.lock().unwrap();
        tracing::info!("Reloading watch paths with config: {:?}", config);
        if let Err(e) = watcher_lock.reload_watch_paths(&config) {
            tracing::warn!("Failed to reload watch paths: {}", e);
        }
    }

    // Update shared config state
    *config_state.lock().unwrap() = config.clone();

    // Enqueue reindex command (processed sequentially by worker thread)
    let task_name = if paths.is_some() { "Path reindex" } else { "Full reindex" };
    let ok = crate::indexer::send_index_command(
        crate::indexer::IndexCommand::Reindex {
            config,
            paths,
            task_name,
        }
    );

    if ok {
        Json(ApiResponse {
            success: true,
            data: Some("Reindex queued".to_string()),
            message: format!("{} queued, check server logs for progress", task_name),
        })
    } else {
        Json(ApiResponse {
            success: false,
            data: None,
            message: "Index worker not initialized".to_string(),
        })
    }
}

#[rocket::post("/remove-paths", data = "<req>")]
pub fn remove_paths(
    req: Json<RemovePathsRequest>,
) -> Json<ApiResponse<String>> {
    let ok = crate::indexer::send_index_command(
        crate::indexer::IndexCommand::RemovePaths(req.paths.clone())
    );

    if ok {
        Json(ApiResponse {
            success: true,
            data: Some("Remove paths queued".to_string()),
            message: "Remove paths queued, check server logs for progress".to_string(),
        })
    } else {
        Json(ApiResponse {
            success: false,
            data: None,
            message: "Index worker not initialized".to_string(),
        })
    }
}

#[rocket::post("/clear")]
pub fn clear(
    db_state: &State<Arc<Database>>,
) -> Json<ApiResponse<String>> {
    match db_state.clear_all() {
        Ok(()) => Json(ApiResponse {
            success: true,
            data: Some("Database cleared".to_string()),
            message: "Database cleared successfully".to_string(),
        }),
        Err(e) => Json(ApiResponse {
            success: false,
            data: None,
            message: e.to_string(),
        }),
    }
}

#[rocket::post("/save-settings")]
pub fn save_settings(
    config_state: &State<Arc<Mutex<Config>>>,
) -> Json<ApiResponse<String>> {
    let config = config_state.lock().unwrap();

    match config.save() {
        Ok(()) => Json(ApiResponse {
            success: true,
            data: Some("Settings saved".to_string()),
            message: "Settings saved successfully".to_string(),
        }),
        Err(e) => Json(ApiResponse {
            success: false,
            data: None,
            message: e.to_string(),
        }),
    }
}

#[rocket::post("/execute-sql", data = "<sql_req>")]
pub fn execute_sql(
    sql_req: Json<ExecuteSqlRequest>,
    db_state: &State<Arc<Database>>,
) -> Json<ApiResponse<SqlResult>> {
    match db_state.execute_custom_sql(&sql_req.sql) {
        Ok(results) => {
            let rows: Vec<Vec<String>> = results.iter().map(|r| r.values.clone()).collect();
            let columns = results.first().map(|r| r.columns.clone()).unwrap_or_default();
            Json(ApiResponse {
                success: true,
                data: Some(SqlResult { columns, rows }),
                message: format!("Executed SQL, returned {} row(s)", results.len()),
            })
        },
        Err(e) => Json(ApiResponse {
            success: false,
            data: None,
            message: e.to_string(),
        }),
    }
}

pub fn rocket(
    db: Arc<Database>,
    config: Arc<Mutex<Config>>,
) -> rocket::Rocket<rocket::Build> {
    rocket::build()
        .attach(AdHoc::on_response("CORS", |_, response| {
            Box::pin(async move {
                response.set_header(rocket::http::Header::new("Access-Control-Allow-Origin", "*"));
                response.set_header(rocket::http::Header::new("Access-Control-Allow-Methods", "GET, POST, OPTIONS"));
                response.set_header(rocket::http::Header::new("Access-Control-Allow-Headers", "Content-Type"));
            })
        }))
        .manage(db)
        .manage(config)
        .mount("/", rocket::routes![
            index,
            search,
            reindex,
            remove_paths,
            clear,
            save_settings,
            execute_sql,
            options_cors
        ])
}

// OPTIONS 预检请求处理器 - 匹配所有路径
#[rocket::options("/<path..>")]
pub fn options_cors(path: PathBuf) -> CorsHeader {
    let _ = path;
    CorsHeader
}
use crate::config::Config;
use crate::database::Database;
use crate::events;
use crate::watcher::FileWatcher;
use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use include_dir::include_dir;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

static FILE_WATCHER: Mutex<Option<Arc<Mutex<FileWatcher>>>> = Mutex::new(None);

pub fn set_file_watcher(watcher: Arc<Mutex<FileWatcher>>) {
    *FILE_WATCHER.lock().unwrap() = Some(watcher);
}

pub fn clear_file_watcher() {
    *FILE_WATCHER.lock().unwrap() = None;
}

pub fn get_file_watcher() -> Option<Arc<Mutex<FileWatcher>>> {
    (*FILE_WATCHER.lock().unwrap()).clone()
}

#[derive(Debug, Deserialize)]
struct RpcRequest {
    #[serde(default)]
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Option<Value>,
}

type RpcResult<T = Value> = Result<T, (i64, String)>;

static FRONTEND_DIR: include_dir::Dir = include_dir!("$CARGO_MANIFEST_DIR/frontend/dist");

#[allow(clippy::too_many_arguments)]
pub async fn serve(
    db: Arc<Database>,
    config: Arc<Mutex<Config>>,
    port: u16,
    http_mode: bool,
    mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
) -> Result<()> {
    let listener = match TcpListener::bind(("127.0.0.1", port)).await {
        Ok(l) => l,
        Err(e) => {
            if e.kind() == std::io::ErrorKind::AddrInUse {
                eprintln!("[backend] port {} already in use, exiting with code 2", port);
                std::process::exit(2);
            }
            return Err(e.into());
        }
    };

    if http_mode {
        tracing::info!("HTTP/WS server listening on http://127.0.0.1:{}", port);
        let url = format!("http://127.0.0.1:{}", port);
        if let Err(e) = opener::open(&url) {
            tracing::warn!(
                "Failed to open browser: {}. Please open {} manually.",
                e, url
            );
        }
    } else {
        tracing::info!(
            "WebSocket JSON-RPC server listening on ws://127.0.0.1:{}",
            port
        );
    }

    #[cfg(unix)]
    let sigterm = {
        use tokio::signal::unix::SignalKind;
        tokio::signal::unix::signal(SignalKind::terminate()).ok()
    };

    let sigterm_fut = async {
        #[cfg(unix)]
        {
            if let Some(mut sig) = sigterm {
                sig.recv().await;
            } else {
                std::future::pending::<()>().await;
            }
        }
        #[cfg(not(unix))]
        {
            std::future::pending::<()>().await;
        }
    };
    tokio::pin!(sigterm_fut);

    loop {
        tokio::select! {
            _ = shutdown_rx.changed() => {
                tracing::info!("Shutdown signal received, server loop exiting");
                break;
            }
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("Ctrl-C received, server loop exiting");
                crate::shutdown::request();
                break;
            }
            _ = &mut sigterm_fut => {
                tracing::info!("SIGTERM received, server loop exiting");
                crate::shutdown::request();
                break;
            }
            accepted = listener.accept() => {
                let (stream, _) = match accepted {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::error!("Accept error: {}", e);
                        continue;
                    }
                };
                let db = Arc::clone(&db);
                let config = Arc::clone(&config);
                tokio::spawn(async move {
                    handle_connection(stream, db, config, http_mode).await;
                });
            }
        }
    }

    Ok(())
}

async fn handle_connection(
    stream: tokio::net::TcpStream,
    db: Arc<Database>,
    config: Arc<Mutex<Config>>,
    http_mode: bool,
) {
    if !http_mode {
        match tokio_tungstenite::accept_async(stream).await {
            Ok(ws) => handle_ws_rpc(ws, db, config).await,
            Err(e) => tracing::debug!("WebSocket handshake failed: {}", e),
        }
        return;
    }

    use tokio::io::AsyncBufReadExt;

    let mut stream = tokio::io::BufReader::new(stream);

    let head = match stream.fill_buf().await {
        Ok(h) => h,
        Err(_) => return,
    };
    if head.is_empty() {
        return;
    }
    let head_str = String::from_utf8_lossy(head);
    let is_ws = head_str.contains("Upgrade:") || head_str.contains("upgrade:");

    if is_ws {
        match tokio_tungstenite::accept_async(stream).await {
            Ok(ws) => handle_ws_rpc(ws, db, config).await,
            Err(e) => tracing::debug!("WebSocket handshake failed: {}", e),
        }
    } else {
        handle_http(stream, db, config).await;
    }
}

async fn handle_http<S>(
    mut stream: S,
    db: Arc<Database>,
    config: Arc<Mutex<Config>>,
) where
    S: AsyncRead + AsyncWrite + Unpin,
{
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut buf = vec![0u8; 8192];
    let mut total = 0;

    loop {
        if total >= buf.len() {
            buf.resize(buf.len() * 2, 0);
        }
        match stream.read(&mut buf[total..]).await {
            Ok(0) => return,
            Ok(n) => {
                total += n;
                if buf[..total].windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            Err(_) => return,
        }
    }

    let request_str = String::from_utf8_lossy(&buf[..total]);
    let first_line = request_str.lines().next().unwrap_or("");
    let parts: Vec<&str> = first_line.split_whitespace().collect();
    let method = parts.get(0).map(|s| *s).unwrap_or("GET");
    let path = parts.get(1).map(|s| *s).unwrap_or("/");

    let body_start = buf[..total]
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|i| i + 4);

    let content_length: usize = request_str
        .lines()
        .find_map(|line| {
            let lower = line.to_lowercase();
            if lower.starts_with("content-length:") {
                lower.split(':').nth(1).and_then(|v| v.trim().parse().ok())
            } else {
                None
            }
        })
        .unwrap_or(0);

    let mut body = Vec::new();
    if let Some(start) = body_start {
        body.extend_from_slice(&buf[start..total]);
    }
    while body.len() < content_length {
        let mut tmp = [0u8; 4096];
        match stream.read(&mut tmp).await {
            Ok(0) => break,
            Ok(n) => body.extend_from_slice(&tmp[..n]),
            Err(_) => break,
        }
    }

    let response = if method == "POST" && path == "/rpc" {
        handle_http_rpc(&body, &db, &config)
    } else if method == "OPTIONS" {
        http_cors_preflight()
    } else {
        serve_static_file(path)
    };

    let _ = stream.write_all(&response).await;
    let _ = stream.flush().await;
}

fn handle_http_rpc(
    body: &[u8],
    db: &Arc<Database>,
    config: &Arc<Mutex<Config>>,
) -> Vec<u8> {
    let resp = match serde_json::from_slice::<RpcRequest>(body) {
        Ok(rpc_req) => {
            let id = rpc_req.id.clone();
            let result = dispatch(&rpc_req, db, config);
            let mut out = json!({ "jsonrpc": "2.0", "id": id });
            match result {
                Ok(r) => out["result"] = r,
                Err((code, msg)) => out["error"] = json!({ "code": code, "message": msg }),
            }
            out
        }
        Err(e) => {
            let body_str = String::from_utf8_lossy(body);
            let preview: String = body_str.chars().take(200).collect();
            tracing::error!("HTTP RPC parse error: {}; body={:?}", e, preview);
            json!({
                "jsonrpc": "2.0",
                "id": null,
                "error": { "code": -32700, "message": "Parse error" }
            })
        }
    };
    let body_str = serde_json::to_string(&resp).unwrap_or_default();
    format_http_response("application/json; charset=utf-8", body_str.as_bytes(), true)
}

fn serve_static_file(path: &str) -> Vec<u8> {
    let file_path = path.trim_start_matches('/');
    let file_path = if file_path.is_empty() {
        "index.html"
    } else {
        file_path
    };

    match FRONTEND_DIR.get_file(file_path) {
        Some(file) => {
            let mime = guess_mime(file_path);
            format_http_response(mime, file.contents(), false)
        }
        None => {
            let status = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n";
            status.as_bytes().to_vec()
        }
    }
}

fn format_http_response(content_type: &str, body: &[u8], cors: bool) -> Vec<u8> {
    let mut header = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-cache\r\n",
        content_type,
        body.len()
    );
    if cors {
        header.push_str("Access-Control-Allow-Origin: *\r\n");
    }
    header.push_str("\r\n");
    let mut resp = header.into_bytes();
    resp.extend_from_slice(body);
    resp
}

fn http_cors_preflight() -> Vec<u8> {
    let header = "HTTP/1.1 204 No Content\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: POST, GET, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\nContent-Length: 0\r\n\r\n";
    header.as_bytes().to_vec()
}

fn guess_mime(path: &str) -> &'static str {
    if path.ends_with(".html") {
        "text/html; charset=utf-8"
    } else if path.ends_with(".js") {
        "application/javascript; charset=utf-8"
    } else if path.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if path.ends_with(".json") {
        "application/json; charset=utf-8"
    } else if path.ends_with(".png") {
        "image/png"
    } else if path.ends_with(".svg") {
        "image/svg+xml"
    } else if path.ends_with(".ico") {
        "image/x-icon"
    } else if path.ends_with(".woff2") {
        "font/woff2"
    } else if path.ends_with(".woff") {
        "font/woff"
    } else if path.ends_with(".ttf") {
        "font/ttf"
    } else {
        "application/octet-stream"
    }
}

async fn handle_ws_rpc<S>(
    ws: tokio_tungstenite::WebSocketStream<S>,
    db: Arc<Database>,
    config: Arc<Mutex<Config>>,
) where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let (mut sink, mut stream) = ws.split();
    let mut notifications = events::subscribe().expect("events channel not initialized");

    loop {
        tokio::select! {
            incoming = stream.next() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        match serde_json::from_str::<RpcRequest>(&text) {
                            Ok(req) => {
                                let id = req.id.clone();
                                let resp = dispatch(&req, &db, &config);
                                if let Some(id) = id {
                                    let mut out = json!({ "jsonrpc": "2.0", "id": id });
                                    match resp {
                                        Ok(result) => { out["result"] = result; }
                                        Err((code, message)) => {
                                            out["error"] = json!({ "code": code, "message": message });
                                        }
                                    }
                                    if sink.send(Message::Text(out.to_string().into())).await.is_err() {
                                        break;
                                    }
                                }
                            }
                            Err(_) => {
                                if let Ok(v) = serde_json::from_str::<Value>(&text) {
                                    if let Some(id) = v.get("id").cloned() {
                                        let out = json!({
                                            "jsonrpc": "2.0",
                                            "id": id,
                                            "error": { "code": -32700, "message": "Parse error" }
                                        });
                                        if sink.send(Message::Text(out.to_string().into())).await.is_err() {
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(e)) => {
                        tracing::debug!("WebSocket error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
            notif = notifications.recv() => {
                match notif {
                    Ok(v) => {
                        if sink.send(Message::Text(v.to_string().into())).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        }
    }
}

fn dispatch(
    req: &RpcRequest,
    db: &Arc<Database>,
    config: &Arc<Mutex<Config>>,
) -> RpcResult {
    let params = req.params.clone().unwrap_or(Value::Null);
    match req.method.as_str() {
        "search" => handle_search(db, &params),
        "execute_sql" => handle_execute_sql(db, &params),
        "reindex" => handle_reindex(config, db, &params),
        "remove_paths" => handle_remove_paths(&params),
        "clear" => handle_clear(db),
        "config_load" => handle_config_load(db),
        "config_save" => handle_config_save(config, db, &params),
        "file_types" => handle_file_types(db),
        "index_status" => handle_index_status(db),
        "shutdown" => handle_shutdown(&params),
        "browse_directory" => handle_browse_directory(&params),
        "open_folder_and_select" => handle_open_folder_and_select(&params),
        "bookmark_add" => handle_bookmark_add(db, &params),
        "bookmark_remove" => handle_bookmark_remove(db, &params),
        "bookmark_list" => handle_bookmark_list(db, &params),
        "bookmark_category_create" => handle_bookmark_category_create(db, &params),
        "bookmark_category_rename" => handle_bookmark_category_rename(db, &params),
        "bookmark_category_delete" => handle_bookmark_category_delete(db, &params),
        "file_content" => handle_file_content(config, db, &params),
        _ => Err((-32601, format!("Method not found: {}", req.method))),
    }
}

fn parse_params<T: for<'de> Deserialize<'de>>(params: &Value) -> Result<T, (i64, String)> {
    serde_json::from_value(params.clone())
        .map_err(|e| (-32602, format!("Invalid params: {}", e)))
}

#[derive(Deserialize)]
#[serde(default)]
struct SearchParams {
    keywords: Vec<String>,
    mode: String,
    sort_by: String,
    context_length: usize,
    page_size: usize,
    last_cursor: Option<crate::database::SearchCursor>,
    filters: Option<crate::database::SearchFilters>,
    #[serde(rename = "name_only")]
    name_only: bool,
}

impl Default for SearchParams {
    fn default() -> Self {
        Self {
            keywords: vec![],
            mode: "OR".into(),
            sort_by: "mtime_desc".into(),
            context_length: 100,
            page_size: 20,
            last_cursor: None,
            filters: None,
            name_only: false,
        }
    }
}

fn handle_search(db: &Arc<Database>, params: &Value) -> RpcResult {
    let p: SearchParams = parse_params(params)?;
    let filters = p.filters.clone().unwrap_or_default();

    let total = match db.count_like(&p.keywords, &p.mode, &filters, p.name_only) {
        Ok(t) => t,
        Err(e) => return Ok(json!({ "success": false, "data": null, "message": e.to_string() })),
    };

    match db.search_like(
        &p.keywords,
        &p.mode,
        &filters,
        p.last_cursor.as_ref(),
        p.sort_by.as_str(),
        p.page_size,
        p.context_length,
        p.name_only,
    ) {
        Ok((hits, next_cursor, has_more)) => {
            let rows: Vec<Vec<String>> = hits
                .iter()
                .map(|h| {
                    vec![
                        h.abs_path.clone(),
                        h.file_size.to_string(),
                        h.context.clone(),
                        h.last_modified_time.clone(),
                    ]
                })
                .collect();
            let next_key = next_cursor.map(|c| json!({
                "sort_value": c.sort_value,
                "path": c.path,
            }));
            Ok(json!({
                "success": true,
                "data": {
                    "columns": ["文件路径", "文件大小", "内容摘要", "修改时间"],
                    "rows": rows,
                    "total": total,
                    "next_key": next_key,
                    "has_more": has_more,
                },
                "message": format!("Found {} row(s)", total),
            }))
        }
        Err(e) => Ok(json!({ "success": false, "data": null, "message": e.to_string() })),
    }
}

#[derive(Deserialize)]
#[serde(default)]
struct ExecuteSqlParams {
    sql: String,
}

impl Default for ExecuteSqlParams {
    fn default() -> Self {
        Self { sql: String::new() }
    }
}

fn handle_execute_sql(db: &Arc<Database>, params: &Value) -> RpcResult {
    let p: ExecuteSqlParams = parse_params(params)?;
    match db.execute_custom_sql(&p.sql) {
        Ok(results) => {
            let rows: Vec<Vec<String>> = results.iter().map(|r| r.values.clone()).collect();
            let columns = results.first().map(|r| r.columns.clone()).unwrap_or_default();
            Ok(json!({
                "success": true,
                "data": { "columns": columns, "rows": rows },
                "message": format!("Executed SQL, returned {} row(s)", rows.len()),
            }))
        }
        Err(e) => Ok(json!({ "success": false, "data": null, "message": e.to_string() })),
    }
}

#[derive(Deserialize)]
#[serde(default)]
struct ReindexParams {
    paths: Option<Vec<String>>,
}

impl Default for ReindexParams {
    fn default() -> Self {
        Self { paths: None }
    }
}

fn handle_reindex(config: &Arc<Mutex<Config>>, db: &Arc<Database>, params: &Value) -> RpcResult {
    let p: ReindexParams = parse_params(params)?;

    let new_config = match Config::load_from_db(db) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Failed to load config from DB, using default: {}", e);
            Config::default()
        }
    };

    if let Some(watcher) = get_file_watcher() {
        if let Err(e) = watcher.lock().unwrap().reload_watch_paths(&new_config) {
            tracing::warn!("Failed to reload watch paths: {}", e);
        }
    }

    *config.lock().unwrap() = new_config.clone();

    let task_name = if p.paths.is_some() {
        "Path reindex"
    } else {
        "Full reindex"
    };
    let ok = crate::indexer::send_index_command(crate::indexer::IndexCommand::Reindex {
        config: new_config,
        paths: p.paths,
        task_name,
    });

    if ok {
        Ok(json!({
            "success": true,
            "data": "Reindex queued",
            "message": format!("{} queued", task_name),
        }))
    } else {
        Ok(json!({ "success": false, "data": null, "message": "Index worker not initialized" }))
    }
}

#[derive(Deserialize)]
#[serde(default)]
struct RemovePathsParams {
    paths: Vec<String>,
}

impl Default for RemovePathsParams {
    fn default() -> Self {
        Self { paths: vec![] }
    }
}

fn handle_remove_paths(params: &Value) -> RpcResult {
    let p: RemovePathsParams = parse_params(params)?;
    let ok = crate::indexer::send_index_command(crate::indexer::IndexCommand::RemovePaths(p.paths));
    if ok {
        Ok(json!({
            "success": true,
            "data": "Remove paths queued",
            "message": "Remove paths queued",
        }))
    } else {
        Ok(json!({ "success": false, "data": null, "message": "Index worker not initialized" }))
    }
}

fn handle_clear(db: &Arc<Database>) -> RpcResult {
    match db.clear_all() {
        Ok(()) => Ok(json!({
            "success": true,
            "data": "Database cleared",
            "message": "Database cleared successfully",
        })),
        Err(e) => Ok(json!({ "success": false, "data": null, "message": e.to_string() })),
    }
}

fn handle_config_load(db: &Arc<Database>) -> RpcResult {
    match Config::load_from_db(db) {
        Ok(config) => Ok(json!({ "success": true, "data": config, "message": "Config loaded" })),
        Err(e) => Ok(json!({ "success": false, "data": null, "message": e.to_string() })),
    }
}

fn handle_file_types(db: &Arc<Database>) -> RpcResult {
    match db.get_file_types() {
        Ok(types) => Ok(json!({ "success": true, "data": types, "message": "" })),
        Err(e) => Ok(json!({ "success": false, "data": null, "message": e.to_string() })),
    }
}

#[derive(Deserialize)]
struct SaveConfigParams {
    config: Config,
}

fn handle_config_save(config_state: &Arc<Mutex<Config>>, db: &Arc<Database>, params: &Value) -> RpcResult {
    let p: SaveConfigParams = parse_params(params)?;
    let old = Config::load_from_db(db).unwrap_or_default();
    let new = p.config;

    if let Err(e) = new.save_to_db(db) {
        return Ok(json!({ "success": false, "data": null, "message": e.to_string() }));
    }

    *config_state.lock().unwrap() = new.clone();

    if let Some(watcher) = get_file_watcher() {
        if let Err(e) = watcher.lock().unwrap().reload_watch_paths(&new) {
            tracing::warn!("Failed to reload watch paths: {}", e);
        }
    }

    let old_paths: Vec<String> = old.watch_paths.iter().map(|w| w.path.clone()).collect();
    let new_paths: Vec<String> = new.watch_paths.iter().map(|w| w.path.clone()).collect();
    let added: Vec<String> = new_paths
        .iter()
        .filter(|p| !old_paths.contains(p))
        .cloned()
        .collect();
    let removed: Vec<String> = old_paths
        .iter()
        .filter(|p| !new_paths.contains(p))
        .cloned()
        .collect();

    if !added.is_empty() {
        crate::indexer::send_index_command(crate::indexer::IndexCommand::Reindex {
            config: new.clone(),
            paths: Some(added.clone()),
            task_name: "Path reindex",
        });
    }
    if !removed.is_empty() {
        crate::indexer::send_index_command(crate::indexer::IndexCommand::RemovePaths(
            removed.clone(),
        ));
    }
    if added.is_empty() && removed.is_empty() {
        crate::indexer::send_index_command(crate::indexer::IndexCommand::Reindex {
            config: new,
            paths: None,
            task_name: "Full reindex",
        });
    }

    Ok(json!({
        "success": true,
        "data": null,
        "message": "Settings saved successfully",
    }))
}

fn handle_index_status(db: &Arc<Database>) -> RpcResult {
    let file_count = db.get_file_count().unwrap_or(0);

    let ts_home = std::env::var("TS_HOME").unwrap_or_else(|_| ".".into());
    let db_path = std::path::Path::new(&ts_home).join("db").join("index.sqlite");
    let index_size_mb = file_size_mb(&db_path);

    let last_index_time = db.get_last_index_time().ok().flatten().unwrap_or_default();

    Ok(json!({
        "success": true,
        "data": {
            "file_count": file_count,
            "index_size_mb": index_size_mb,
            "last_index_time": last_index_time,
            "is_indexing": crate::indexer::is_indexing(),
        },
        "message": ""
    }))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ShutdownParams {
    force: bool,
}

fn handle_shutdown(params: &Value) -> RpcResult {
    let p: ShutdownParams = parse_params(params)?;

    // 非强制且正在索引：返回失败+is_indexing，后端不实际 shutdown。
    if !p.force && crate::indexer::is_indexing() {
        return Ok(json!({
            "success": false,
            "data": { "is_indexing": true },
            "message": "Indexing in progress"
        }));
    }

    tracing::info!("RPC shutdown requested (force={})", p.force);
    crate::shutdown::request();
    Ok(json!({
        "success": true,
        "data": { "is_indexing": false },
        "message": "Shutdown initiated",
    }))
}

fn file_size_mb(path: &std::path::Path) -> f64 {
    std::fs::metadata(path)
        .map(|m| m.len() as f64 / (1024.0 * 1024.0))
        .unwrap_or(0.0)
}

#[derive(Deserialize)]
#[serde(default)]
struct BrowseDirectoryParams {
    default_path: Option<String>,
}

impl Default for BrowseDirectoryParams {
    fn default() -> Self {
        Self { default_path: None }
    }
}

fn handle_browse_directory(params: &Value) -> RpcResult {
    let p: BrowseDirectoryParams = parse_params(params)?;
    let start_dir = p.default_path.filter(|s| !s.is_empty()).unwrap_or_default();

    #[cfg(target_os = "windows")]
    {
        let ps = r#"
Add-Type -AssemblyName System.Windows.Forms
$f = New-Object System.Windows.Forms.FolderBrowserDialog
$f.Description = 'Select Directory'
if (![string]::IsNullOrEmpty($env:TS_START_DIR)) { $f.SelectedPath = $env:TS_START_DIR }
if ($f.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
    Write-Output $f.SelectedPath
}
"#;
        let output = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", ps])
            .env("TS_START_DIR", &start_dir)
            .output();

        match output {
            Ok(o) if o.status.success() => {
                let path = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if path.is_empty() {
                    Ok(json!({ "success": true, "data": null, "message": "Cancelled" }))
                } else {
                    Ok(json!({ "success": true, "data": path, "message": "Selected" }))
                }
            }
            Ok(_) => Ok(json!({ "success": true, "data": null, "message": "Cancelled" })),
            Err(e) => {
                tracing::warn!("powershell dialog failed: {}", e);
                Ok(json!({
                    "success": false,
                    "data": null,
                    "message": "No file dialog available. Please enter the path manually."
                }))
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        let script = format!(
            "POSIX path of (choose folder with prompt \"Select Directory\"{})",
            if start_dir.is_empty() {
                String::new()
            } else {
                format!(" default location \"{}\"", start_dir)
            }
        );
        let output = std::process::Command::new("osascript")
            .args(["-e", &script])
            .output();

        match output {
            Ok(o) if o.status.success() => {
                let path = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if path.is_empty() {
                    Ok(json!({ "success": true, "data": null, "message": "Cancelled" }))
                } else {
                    Ok(json!({ "success": true, "data": path, "message": "Selected" }))
                }
            }
            Ok(_) => Ok(json!({ "success": true, "data": null, "message": "Cancelled" })),
            Err(e) => {
                tracing::warn!("osascript dialog failed: {}", e);
                Ok(json!({
                    "success": false,
                    "data": null,
                    "message": "No file dialog available. Please enter the path manually."
                }))
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        let output = std::process::Command::new("zenity")
            .args(["--file-selection", "--directory"])
            .arg(if start_dir.is_empty() {
                String::new()
            } else {
                format!("--filename={}/", start_dir)
            })
            .output();

        match output {
            Ok(o) if o.status.success() => {
                let path = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if path.is_empty() {
                    Ok(json!({ "success": true, "data": null, "message": "Cancelled" }))
                } else {
                    Ok(json!({ "success": true, "data": path, "message": "Selected" }))
                }
            }
            Ok(_) => Ok(json!({ "success": true, "data": null, "message": "Cancelled" })),
            Err(e) => {
                tracing::warn!("zenity not available: {}", e);
                Ok(json!({
                    "success": false,
                    "data": null,
                    "message": "No file dialog available. Please enter the path manually."
                }))
            }
        }
    }
}

#[derive(Deserialize)]
struct OpenFolderAndSelectParams {
    path: String,
}

fn handle_open_folder_and_select(params: &Value) -> RpcResult {
    let p: OpenFolderAndSelectParams = parse_params(params)?;
    let path = std::path::Path::new(&p.path);

    if cfg!(target_os = "linux") {
        if let Some(parent) = path.parent() {
            if let Err(e) = opener::open(parent) {
                return Ok(json!({ "success": false, "data": null, "message": e.to_string() }));
            }
        }
        return Ok(json!({ "success": true, "data": null, "message": "Opened" }));
    }

    if cfg!(target_os = "macos") {
        let _ = std::process::Command::new("open")
            .args(["-R", &p.path])
            .spawn();
        return Ok(json!({ "success": true, "data": null, "message": "Opened" }));
    }

    if cfg!(target_os = "windows") {
        let _ = std::process::Command::new("explorer")
            .arg(format!("/select,\"{}\"", p.path))
            .spawn();
        return Ok(json!({ "success": true, "data": null, "message": "Opened" }));
    }

    Ok(json!({ "success": false, "data": null, "message": "Unsupported platform" }))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct BookmarkAddParams {
    abs_path: String,
    name: String,
    file_size: Option<i64>,
    last_modified_time: Option<String>,
    category_id: Option<i64>,
    category_name: Option<String>,
}

fn handle_bookmark_add(db: &Arc<Database>, params: &Value) -> RpcResult {
    let p: BookmarkAddParams = parse_params(params)?;
    let cat_id = match p.category_id {
        Some(id) => id,
        None => {
            let name = p.category_name.as_deref().unwrap_or("默认");
            db.add_bookmark_category(name)
                .map_err(|e| (-1, e.to_string()))?
        }
    };
    db.add_bookmark(cat_id, &p.abs_path, &p.name, p.file_size, p.last_modified_time.as_deref())
        .map_err(|e| (-1, e.to_string()))?;
    Ok(json!({ "success": true, "data": null, "message": "Bookmarked" }))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct BookmarkRemoveParams {
    id: Option<i64>,
    abs_path: Option<String>,
    category_id: Option<i64>,
}

fn handle_bookmark_remove(db: &Arc<Database>, params: &Value) -> RpcResult {
    let p: BookmarkRemoveParams = parse_params(params)?;
    if let Some(id) = p.id {
        db.remove_bookmark(id).map_err(|e| (-1, e.to_string()))?;
    } else if let (Some(cat_id), Some(path)) = (p.category_id, p.abs_path.as_ref()) {
        db.remove_bookmark_by_path(cat_id, path).map_err(|e| (-1, e.to_string()))?;
    }
    Ok(json!({ "success": true, "data": null, "message": "Removed" }))
}

fn handle_bookmark_list(db: &Arc<Database>, _params: &Value) -> RpcResult {
    match db.list_bookmarks() {
        Ok(categories) => Ok(json!({ "success": true, "data": categories, "message": "" })),
        Err(e) => Ok(json!({ "success": false, "data": null, "message": e.to_string() })),
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct BookmarkCategoryCreateParams {
    name: String,
}

fn handle_bookmark_category_create(db: &Arc<Database>, params: &Value) -> RpcResult {
    let p: BookmarkCategoryCreateParams = parse_params(params)?;
    let id = db.add_bookmark_category(&p.name).map_err(|e| (-1, e.to_string()))?;
    Ok(json!({ "success": true, "data": { "id": id }, "message": "Created" }))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct BookmarkCategoryRenameParams {
    id: i64,
    name: String,
}

fn handle_bookmark_category_rename(db: &Arc<Database>, params: &Value) -> RpcResult {
    let p: BookmarkCategoryRenameParams = parse_params(params)?;
    db.rename_bookmark_category(p.id, &p.name).map_err(|e| (-1, e.to_string()))?;
    Ok(json!({ "success": true, "data": null, "message": "Renamed" }))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct BookmarkCategoryDeleteParams {
    id: i64,
}

fn handle_bookmark_category_delete(db: &Arc<Database>, params: &Value) -> RpcResult {
    let p: BookmarkCategoryDeleteParams = parse_params(params)?;
    db.delete_bookmark_category(p.id).map_err(|e| (-1, e.to_string()))?;
    Ok(json!({ "success": true, "data": null, "message": "Deleted" }))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct FileContentParams {
    abs_path: String,
}

fn handle_file_content(config: &Arc<Mutex<Config>>, db: &Arc<Database>, params: &Value) -> RpcResult {
    let p: FileContentParams = parse_params(params)?;
    let preview_length = config.lock().unwrap().preview_length.max(100);
    match db.get_file_content_preview(&p.abs_path, preview_length) {
        Ok(Some((name, file_size, mtime, content, truncated))) => Ok(json!({
            "success": true,
            "data": {
                "name": name,
                "file_size": file_size,
                "last_modified_time": mtime,
                "content": content,
                "truncated": truncated,
            },
            "message": "",
        })),
        Ok(None) => Ok(json!({ "success": false, "data": null, "message": "File not found in index" })),
        Err(e) => Ok(json!({ "success": false, "data": null, "message": e.to_string() })),
    }
}

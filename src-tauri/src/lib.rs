use std::fs::{File, OpenOptions};
use std::io::Write;
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Emitter, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_shell::process::CommandChild;
use tauri_plugin_shell::ShellExt;

struct BackendState {
    child: Mutex<Option<CommandChild>>,
    /// 子进程是否已退出（Terminated 事件驱动）。
    exited: Arc<AtomicBool>,
}

/// 当前后端监听端口（宿主内存状态，重试换端口时更新）。
struct PortState(Mutex<u16>);

fn resolve_ts_home(_app: &AppHandle) -> PathBuf {
    // 与后端一致：可执行文件所在目录作为部署根目录
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// 在 10000~60000 之间随机选一个空闲端口，最多尝试三次。
fn pick_port() -> Option<u16> {
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

#[tauri::command]
fn get_server_port(state: tauri::State<PortState>) -> Result<u16, String> {
    Ok(*state.0.lock().unwrap())
}

#[tauri::command]
fn backend_exited(state: tauri::State<BackendState>) -> Result<bool, String> {
    Ok(state.exited.load(Ordering::SeqCst))
}

#[tauri::command]
fn force_kill_backend(app: AppHandle) -> Result<(), String> {
    let state = app.state::<BackendState>();
    if let Some(child) = state.child.lock().unwrap().take() {
        child.kill().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn exit_app(app: AppHandle) {
    app.exit(0);
}

fn open_log_file(ts_home: &PathBuf) -> File {
    let log_dir = ts_home.join("logs");
    let _ = std::fs::create_dir_all(&log_dir);
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_dir.join("backend.log"))
        .unwrap_or_else(|e| {
            eprintln!(
                "[backend] cannot open log file: {}, falling back to temp",
                e
            );
            File::create(std::env::temp_dir().join("text_search_backend.log"))
                .expect("cannot create fallback log")
        })
}

fn launch_sidecar(
    app: &AppHandle,
    port: u16,
) -> Result<
    (
        tauri::async_runtime::Receiver<tauri_plugin_shell::process::CommandEvent>,
        CommandChild,
    ),
    String,
> {
    let sidecar = app
        .shell()
        .sidecar("text_search")
        .map_err(|e| e.to_string())?;
    let port_str = port.to_string();
    sidecar
        .args(["--port", port_str.as_str()])
        .env("RUST_LOG", "info,lopdf_parang=error")
        .env("TS_LOG_CONSOLE", "0")
        .env("TS_LAUNCH_MODE", "tauri")
        .spawn()
        .map_err(|e| e.to_string())
}

fn spawn_backend(app: &AppHandle) -> Result<CommandChild, String> {
    let ts_home = resolve_ts_home(app);
    let log_file = open_log_file(&ts_home);
    let port = *app.state::<PortState>().0.lock().unwrap();

    let (rx, child) = launch_sidecar(app, port)?;
    app.state::<BackendState>()
        .exited
        .store(false, Ordering::SeqCst);

    let app_handle = app.clone();
    let exited = Arc::clone(&app.state::<BackendState>().exited);
    tauri::async_runtime::spawn(async move {
        use tauri_plugin_shell::process::CommandEvent;
        let mut log = log_file;
        let mut rx = rx;
        let mut attempts = 0;

        loop {
            let mut respawn = false;
            while let Some(event) = rx.recv().await {
                match event {
                    CommandEvent::Stdout(line_bytes) => {
                        let line = String::from_utf8_lossy(&line_bytes);
                        let _ = writeln!(log, "[stdout] {}", line.trim_end());
                    }
                    CommandEvent::Stderr(line_bytes) => {
                        let line = String::from_utf8_lossy(&line_bytes);
                        let _ = writeln!(log, "[stderr] {}", line.trim_end());
                    }
                    CommandEvent::Error(e) => {
                        let _ = writeln!(log, "[error] {}", e);
                        eprintln!("[backend] sidecar error: {}", e);
                    }
                    CommandEvent::Terminated(payload) => {
                        let _ = writeln!(log, "[terminated] code={:?}", payload.code);
                        eprintln!("[backend] sidecar terminated: {:?}", payload.code);
                        // 退出码 2 = 端口冲突，由后端约定；宿主重新计算端口并重试。
                        if payload.code == Some(2) && attempts < 5 {
                            respawn = true;
                        }
                        break;
                    }
                    _ => {}
                }
            }

            if !respawn {
                exited.store(true, Ordering::SeqCst);
                let _ = app_handle.emit("backend-terminated", ());
                break;
            }

            attempts += 1;
            eprintln!("[backend] port conflict, retrying ({}/5)...", attempts);
            let new_port = match pick_port() {
                Some(p) => p,
                None => {
                    eprintln!("[backend] no free port available, giving up");
                    break;
                }
            };
            *app_handle.state::<PortState>().0.lock().unwrap() = new_port;
            std::thread::sleep(Duration::from_millis(300));

            match launch_sidecar(&app_handle, new_port) {
                Ok((new_rx, new_child)) => {
                    exited.store(false, Ordering::SeqCst);
                    *app_handle.state::<BackendState>().child.lock().unwrap() = Some(new_child);
                    rx = new_rx;
                }
                Err(e) => {
                    eprintln!("[backend] respawn failed: {}", e);
                    break;
                }
            }
        }
    });

    Ok(child)
}

fn wait_for_backend(app: &AppHandle) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let port = *app.state::<PortState>().0.lock().unwrap();
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return;
        }
        if Instant::now() >= deadline {
            eprintln!("[backend] timeout waiting for ws server");
            return;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

#[tauri::command]
async fn browse_directory(
    app: AppHandle,
    default_path: Option<String>,
) -> Result<Option<String>, String> {
    use std::sync::mpsc;
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = mpsc::channel::<Option<tauri_plugin_dialog::FilePath>>();
    let mut builder = app.dialog().file().set_title("Select Directory");
    if let Some(p) = default_path.filter(|p| !p.is_empty()) {
        builder = builder.set_directory(&p);
    }
    builder.pick_folder(move |path| {
        let _ = tx.send(path);
    });
    let result = tauri::async_runtime::spawn_blocking(move || rx.recv())
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    Ok(result
        .and_then(|f: tauri_plugin_dialog::FilePath| f.into_path().ok())
        .map(|p: std::path::PathBuf| p.to_string_lossy().to_string()))
}

#[tauri::command]
async fn open_folder_and_select(app: AppHandle, path: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .reveal_item_in_dir(&path)
        .map_err(|e| e.to_string())
}

fn setup_menu(app: &AppHandle) -> tauri::Result<()> {
    let settings = MenuItem::with_id(app, "settings", "Settings", true, Some("CmdOrCtrl+,"))?;
    let exit = MenuItem::with_id(app, "exit", "Exit", true, Some("CmdOrCtrl+Q"))?;
    let file_menu = Submenu::with_items(
        app,
        "File",
        true,
        &[&settings, &PredefinedMenuItem::separator(app)?, &exit],
    )?;

    let undo = PredefinedMenuItem::undo(app, None)?;
    let redo = PredefinedMenuItem::redo(app, None)?;
    let cut = PredefinedMenuItem::cut(app, None)?;
    let copy = PredefinedMenuItem::copy(app, None)?;
    let paste = PredefinedMenuItem::paste(app, None)?;
    let select_all = PredefinedMenuItem::select_all(app, None)?;
    let edit_menu = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &undo,
            &redo,
            &PredefinedMenuItem::separator(app)?,
            &cut,
            &copy,
            &paste,
            &PredefinedMenuItem::separator(app)?,
            &select_all,
        ],
    )?;

    let reload = MenuItem::with_id(app, "reload", "Reload", true, Some("CmdOrCtrl+R"))?;
    let devtools = MenuItem::with_id(app, "devtools", "Toggle DevTools", true, Some("F12"))?;
    let zoom_in = MenuItem::with_id(app, "zoom_in", "Zoom In", true, Some("CmdOrCtrl+Plus"))?;
    let zoom_out = MenuItem::with_id(app, "zoom_out", "Zoom Out", true, Some("CmdOrCtrl+-"))?;
    let zoom_reset = MenuItem::with_id(app, "zoom_reset", "Reset Zoom", true, Some("CmdOrCtrl+0"))?;
    let view_menu = Submenu::with_items(
        app,
        "View",
        true,
        &[
            &reload,
            &devtools,
            &PredefinedMenuItem::separator(app)?,
            &zoom_in,
            &zoom_out,
            &zoom_reset,
        ],
    )?;

    let about = MenuItem::with_id(app, "about", "About", true, None::<&str>)?;
    let help_menu = Submenu::with_items(app, "Help", true, &[&about])?;

    let menu = Menu::with_items(app, &[&file_menu, &edit_menu, &view_menu, &help_menu])?;
    app.set_menu(menu)?;
    Ok(())
}

fn zoom(app: &AppHandle, delta: f64) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.eval(&format!(
            "window.__zoom = (window.__zoom || 1) + {}; document.body.style.zoom = window.__zoom;",
            delta
        ));
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            browse_directory,
            open_folder_and_select,
            get_server_port,
            backend_exited,
            force_kill_backend,
            exit_app
        ])
        .on_menu_event(|app, event| match event.id().as_ref() {
            "settings" => {
                let _ = app.emit("open-settings", ());
            }
            "exit" => app.exit(0),
            "reload" => {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.eval("location.reload()");
                }
            }
            "devtools" => {
                if let Some(w) = app.get_webview_window("main") {
                    #[cfg(debug_assertions)]
                    let _ = w.open_devtools();
                    #[cfg(not(debug_assertions))]
                    let _ = w;
                }
            }
            "zoom_in" => zoom(app, 0.1),
            "zoom_out" => zoom(app, -0.1),
            "zoom_reset" => {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.eval("window.__zoom = 1; document.body.style.zoom = 1;");
                }
            }
            "about" => {
                use tauri_plugin_dialog::DialogExt;
                let _ = app
                    .dialog()
                    .message("Text Search Application\nA powerful text search tool for documents.")
                    .title("Text Search")
                    .blocking_show();
            }
            _ => {}
        })
        .setup(|app| {
            setup_menu(app.handle())?;

            let port = match pick_port() {
                Some(p) => p,
                None => {
                    eprintln!("[backend] no free port available in 10000-60000");
                    return Err("no free port available".into());
                }
            };

            app.manage(PortState(Mutex::new(port)));
            app.manage(BackendState {
                child: Mutex::new(None),
                exited: Arc::new(AtomicBool::new(true)),
            });

            match spawn_backend(app.handle()) {
                Ok(child) => {
                    *app.state::<BackendState>().child.lock().unwrap() = Some(child);
                }
                Err(e) => eprintln!("[backend] failed to spawn sidecar: {}", e),
            }

            let window = WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
                .title("Text Search")
                .inner_size(900.0, 700.0)
                .min_inner_size(400.0, 120.0)
                .resizable(true)
                .decorations(false)
                .visible(false)
                .maximized(true)
                .build()?;

            let win = window.clone();
            let app_handle = app.handle().clone();
            std::thread::spawn(move || {
                wait_for_backend(&app_handle);
                let _ = win.show();
                let _ = win.set_focus();
            });

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| match event {
            RunEvent::ExitRequested { api, .. } => {
                let state = app_handle.try_state::<BackendState>();
                let exited = state
                    .as_ref()
                    .map(|s| s.exited.load(Ordering::SeqCst))
                    .unwrap_or(true);
                let child_gone = state
                    .map(|s| s.child.lock().unwrap().is_none())
                    .unwrap_or(true);
                // 后端已被强制杀死（child 已 take）或已优雅退出，直接退出。
                if !exited && !child_gone {
                    api.prevent_exit();
                    let app = app_handle.clone();
                    std::thread::spawn(move || {
                        let deadline = Instant::now() + Duration::from_secs(30);
                        loop {
                            let done = app
                                .try_state::<BackendState>()
                                .map(|s| s.exited.load(Ordering::SeqCst))
                                .unwrap_or(true);
                            if done || Instant::now() >= deadline {
                                break;
                            }
                            std::thread::sleep(Duration::from_millis(100));
                        }
                        app.exit(0);
                    });
                }
            }
            RunEvent::Exit => {
                if let Some(state) = app_handle.try_state::<BackendState>() {
                    if let Some(child) = state.child.lock().unwrap().take() {
                        let _ = child.kill();
                    }
                }
            }
            _ => {}
        });
}

use anyhow::Result;
use std::path::{Path, PathBuf};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

/// 检测当前系统的字符集编码并配置控制台输出。
///
/// - **Windows**: 读取当前控制台代码页（如 936=GBK, 65001=UTF-8），
///   将控制台输出代码页设为 UTF-8，并将 stdout 设为二进制模式
///   （防止 CRT 在写入管道时对 UTF-8 进行 ANSI 代码页转换）。
/// - **其他平台**: 仅打印系统 locale 信息（通常为 UTF-8）。
fn setup_console_encoding() {
    #[cfg(windows)]
    {
        extern "system" {
            fn GetConsoleOutputCP() -> u32;
            fn SetConsoleOutputCP(wCodePageID: u32) -> i32;
        }
        extern "C" {
            fn _setmode(_fd: i32, _mode: i32) -> i32;
        }
        const CP_UTF8: u32 = 65001;
        const _O_BINARY: i32 = 0x8000;

        unsafe {
            let original_cp = GetConsoleOutputCP();
            eprintln!(
                "[init] Windows console output code page: {} {}",
                original_cp,
                code_page_name(original_cp)
            );

            if SetConsoleOutputCP(CP_UTF8) == 0 {
                eprintln!("[init] WARN: Failed to set console to UTF-8");
            } else {
                eprintln!("[init] Console output code page set to UTF-8 (65001)");
            }

            _setmode(1, _O_BINARY);
        }
    }

    #[cfg(not(windows))]
    {
        eprintln!(
            "[init] System charset: UTF-8 ({})",
            std::env::var("LANG").unwrap_or_default()
        );
    }
}

/// 返回代码页对应的名称，方便日志阅读。
#[cfg(windows)]
fn code_page_name(cp: u32) -> &'static str {
    match cp {
        65001 => "(UTF-8)",
        936 => "(GBK, Chinese Simplified)",
        950 => "(BIG5, Chinese Traditional)",
        932 => "(Shift-JIS, Japanese)",
        949 => "(EUC-KR, Korean)",
        1252 => "(Windows-1252, Western Europe)",
        437 => "(OEM-US)",
        855 => "(OEM-Cyrillic)",
        866 => "(OEM-Russian)",
        20866 => "(KOI8-R)",
        28591 => "(ISO-8859-1)",
        _ => "(unknown)",
    }
}

pub fn init_logging() -> Result<()> {
    // TS_LOG_CONSOLE=0 时只写文件（由 Tauri 拉起后端时设置），其余情况保留控制台输出。
    let console_enabled = std::env::var("TS_LOG_CONSOLE")
        .map(|v| v != "0")
        .unwrap_or(true);

    if console_enabled {
        setup_console_encoding();
    }

    // 日志目录：TS_HOME/logs（TS_HOME 未设置时退回 exe 所在目录）
    let log_dir = match std::env::var("TS_HOME") {
        Ok(home) if !home.trim().is_empty() => PathBuf::from(home).join("logs"),
        _ => std::env::current_exe()
            .unwrap_or_else(|_| PathBuf::from("."))
            .parent()
            .unwrap_or(Path::new("."))
            .join("logs"),
    };

    std::fs::create_dir_all(&log_dir)?;

    let file_appender = RollingFileAppender::new(Rotation::DAILY, &log_dir, "text_search");

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,lopdf_parang=error"));

    let file_layer = fmt::layer()
        .with_writer(file_appender)
        .with_ansi(false)
        .with_filter(env_filter.clone());

    let subscriber = tracing_subscriber::registry().with(file_layer);

    if console_enabled {
        let console_layer = fmt::layer()
            .with_writer(std::io::stdout)
            .with_ansi(false)
            .with_filter(env_filter);
        subscriber.with(console_layer).init();
    } else {
        subscriber.init();
    }

    std::panic::set_hook(Box::new(|info| {
        let payload = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "unknown".to_string());
        let loc = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_default();
        tracing::error!("PANIC: {} at {}", payload, loc);
    }));

    Ok(())
}

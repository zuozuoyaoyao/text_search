use anyhow::Result;
use std::path::Path;
use std::path::PathBuf;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{
    fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer,
};

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

            // stdout 设为二进制模式，防止 CRT 将 UTF-8 转换为 ANSI 代码页
            _setmode(1, _O_BINARY);
        }
    }

    #[cfg(not(windows))]
    {
        // 其他平台默认使用 UTF-8
        eprintln!("[init] System charset: UTF-8 ({})", std::env::var("LANG").unwrap_or_default());
    }
}

/// 返回代码页对应的名称，方便日志阅读
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
    // 检测系统字符集编码，配置控制台为 UTF-8 输出
    setup_console_encoding();

    // Get log directory
    let exe_path = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
    let log_dir = exe_path
        .parent()
        .unwrap_or(Path::new("."))
        .join("logs");

    std::fs::create_dir_all(&log_dir)?;

    // Rolling file appender - daily rotation
    let file_appender = RollingFileAppender::new(Rotation::DAILY, &log_dir, "text_search");

    // Environment filter for log level
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    // File layer
    let file_layer = fmt::layer()
        .with_writer(file_appender)
        .with_ansi(false)
        .with_filter(env_filter.clone());

    // Console layer - disable ANSI to avoid encoding issues when spawned from Electron
    let console_layer = fmt::layer()
        .with_writer(std::io::stdout)
        .with_ansi(false)  // 禁用 ANSI 转义码，让 Node.js 能正确解析 UTF-8
        .with_filter(env_filter);

    tracing_subscriber::registry()
        .with(file_layer)
        .with(console_layer)
        .init();

    Ok(())
}

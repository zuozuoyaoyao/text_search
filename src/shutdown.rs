use std::sync::OnceLock;
use tokio::sync::watch;

static TX: OnceLock<watch::Sender<bool>> = OnceLock::new();

/// 初始化退出信号通道，须在 rpc::serve 前调用。返回接收端。
pub fn init() -> watch::Receiver<bool> {
    let (tx, rx) = watch::channel(false);
    let _ = TX.set(tx);
    rx
}

/// 请求优雅退出（RPC shutdown / 信号处理程序调用）。
pub fn request() {
    if let Some(tx) = TX.get() {
        let _ = tx.send(true);
    }
}

/// 是否已请求退出（供非异步线程轮询，如索引 worker）。
pub fn requested() -> bool {
    TX.get().map(|tx| *tx.borrow()).unwrap_or(false)
}

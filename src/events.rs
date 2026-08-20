use serde_json::{json, Value};
use std::sync::Mutex;
use tokio::sync::broadcast;

static TX: Mutex<Option<broadcast::Sender<Value>>> = Mutex::new(None);

/// 初始化通知广播通道，须在索引 worker 启动前调用。
pub fn init() {
    let (tx, _) = broadcast::channel(64);
    *TX.lock().unwrap() = Some(tx);
}

/// 向所有已连接的客户端广播 JSON-RPC 通知。
pub fn emit(method: &str, params: Value) {
    if let Some(tx) = TX.lock().unwrap().as_ref() {
        let msg = json!({ "jsonrpc": "2.0", "method": method, "params": params });
        let _ = tx.send(msg);
    }
}

/// 订阅通知（每个 WS 连接调用一次）。
pub fn subscribe() -> Option<broadcast::Receiver<Value>> {
    TX.lock().unwrap().as_ref().map(|tx| tx.subscribe())
}

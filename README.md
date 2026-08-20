# Text Search

基于 Rust + Vue 3 的本地文档全文搜索桌面应用（Tauri 2）。

## 架构

- **后端**（`text_search`，独立进程）：DuckDB 索引 + 文件解析/监听 + 索引 worker，对外提供
  WebSocket + JSON-RPC 2.0 服务（默认 `ws://127.0.0.1:8899`，可用环境变量 `TS_WS_PORT` 覆盖）。
- **Tauri 壳**（`src-tauri/`）：窗口、原生菜单、对话框、单实例；启动时以 sidecar 拉起后端，
  stdout/stderr 重定向到 `TS_HOME/logs/backend.log`，就绪后显示窗口。
- **前端**（`frontend/`）：Vue 3 + Vite + Arco Design Vue，Everything 风格精简/高级双模式，
  通过 WebSocket 直连后端。

## 数据目录（TS_HOME）

- 开发模式（debug 构建）：项目根目录（`config.toml`、`index.duckdb`、`logs/` 都在这里）。
- 生产模式（release 构建）：系统应用数据目录，Windows 下为
  `%APPDATA%\com.blackyao.text_search`。

## 开发

```bash
cd frontend
npm install
npm run tauri:dev
```

`tauri:dev` 会先构建后端 sidecar（`scripts/build-sidecar.mjs`），再启动 Vite 与 Tauri。

仅调试后端（不启动界面）：

```bash
cargo run --features with-ws-server
```

后端日志：控制台（`TS_LOG_CONSOLE=0` 时仅写文件）与 `TS_HOME/logs/text_search*.log`。
Windows 下查看日志建议用 VS Code 或 `Get-Content -Encoding UTF8`。

## 构建安装包

每平台一个打包脚本，三个模式：`backend`（无 Tauri 版，独立后端+内置 Web UI）、
`tauri`（仅 Tauri 桌面版）、`all`（都包含）。无参数或 `--help` 打印帮助。

Linux：

```bash
./build.sh backend    # 仅无 Tauri 版
./build.sh tauri      # 仅 Tauri 版
./build.sh all        # 都包含
```

Windows：

```bat
build.bat backend
build.bat tauri
build.bat all
```

产出在 `dist/`：Linux 为 `TextSearch-v<ver>-linux-x64[-backend|-tauri].tar.gz`，
Windows 为 `TextSearch-v<ver>-win64[-backend|-tauri].zip`，构建完成后会打印完整路径。

## 主要 RPC 方法

`search`（多关键词 AND/OR、上下文截取、LIMIT）、`execute_sql`、`reindex`、`remove_paths`、
`clear`、`config_load`、`config_save`；服务端通知：`index_completed`、`index_error`。

## Electron 旧版

迁移前的 Electron 双进程版本保留在 `master` 分支（标签 `electron-v1`）：

```bash
git checkout master
```

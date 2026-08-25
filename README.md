# Text Search（Electron 旧版）

## 1. 简介

`electron-ui` 是 Text Search 在迁移到 Tauri 之前的桌面版本，采用 Electron + Vue 3 前端与 Rust + Rocket 后端双进程架构。后端负责解析文档、建立索引、监听文件变化并提供 HTTP API，Electron 负责窗口和界面。支持 TXT、Markdown、CSV、RTF、PDF、DOCX、PPTX、XLS/XLSX、ODT、ODS 和 ODP。

该分支用于兼容旧部署；新功能和新发布版本请优先使用 `master`/`tauri-ui` 的 Tauri 主线。

## 2. 安装部署

请从 [GitHub Releases](https://github.com/zuozuoyaoyao/text_search/releases) 下载与系统匹配的 Electron 构建包。

解压后运行生成的 Electron 应用即可。应用会启动本地 Rust 后端，默认使用 `http://127.0.0.1:8000` 提供搜索服务；运行目录中的 `config.toml` 或 `TS_HOME` 指向的数据目录用于保存监控路径、文件类型和索引配置。

## 3. 功能描述

### 设置

旧版设置对话框可以调整上下文长度、最大结果数，选择需要索引的文件类型，并添加或删除监控路径。监控路径可设置是否递归扫描子目录；保存后配置写入 `config.toml`。

### 搜索

在搜索框输入关键词后回车或点击 Search。多个关键词支持 **AND** 和 **OR** 模式，结果表格展示文件路径、内容摘要和其他索引字段；摘要会高亮关键词，点击文件路径可在文件管理器中定位文件。结果数量可由界面中的 Max Results 限制。

### 收藏

Electron 旧版没有 Tauri 主线中的收藏栏和分类收藏功能。如需收藏文件，可使用操作系统文件管理器或升级到 `master`/`tauri-ui`。

### 菜单（索引）

旧版将索引操作直接放在主界面按钮中：**Reindex All** 重新扫描所有监控路径，**Clear All** 清空索引记录但不会删除源文件。配置中的文件类型和监控路径决定后续索引范围。

### 专业模式

勾选 **Show SQL** 可显示 SQL 面板。搜索生成的 SQL 会填入编辑框，也可以直接修改后点击 Execute 查询；结果以表格展示，适合排查索引内容和进行高级筛选。该模式是旧版的 SQL 调试界面，不等同于 Tauri 主线的专业模式体验。

## 4. 源码编译

Windows：

```bat
build_all.bat release
```

Linux：

```bash
bash ./build_all.sh release
```

如需分别构建，也可以使用：

```bash
bash ./build_backend.sh release
cd frontend && bash ./build_frontend.sh release
```

构建输出位于 `frontend/dist_electron/`，Rust 后端位于 `target/release/`。构建脚本会把后端复制到临时资源目录后再执行 Electron 打包，生成的后端二进制不应提交到 Git。

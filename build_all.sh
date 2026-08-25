#!/bin/bash
# 整体构建脚本：将前后端分别打包成独立的可执行文件
# 双进程架构：
#   - 后端：Rust 可执行文件 (target/debug/text_search 或 target/release/text_search)
#   - 前端：Electron 应用 (frontend/dist_electron/)

set -e

echo "========================================"
echo "  构建前后端双进程独立可执行文件"
echo "========================================"

# 获取脚本所在目录作为项目根目录
PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FRONTEND_DIR="$PROJECT_DIR/frontend"

BUILD_MODE="${1:-release}"

if [ "$BUILD_MODE" = "release" ]; then
    BACKEND_PATH="$PROJECT_DIR/target/release/text_search"
    CARGO_BUILD_CMD="cargo build --release --features with-http-server"
else
    BACKEND_PATH="$PROJECT_DIR/target/debug/text_search"
    CARGO_BUILD_CMD="cargo build --features with-http-server"
fi

echo ""
echo "========================================"
echo "  第一部分：构建 Rust 后端 ($BUILD_MODE 模式)"
echo "========================================"

echo ""
echo "步骤 1/3: 清理旧的构建产物..."
rm -f "$PROJECT_DIR/target/debug/text_search"
rm -f "$PROJECT_DIR/target/release/text_search"
rm -f "$PROJECT_DIR/target/debug/text_search.exe"
rm -f "$PROJECT_DIR/target/release/text_search.exe"

echo ""
echo "步骤 2/3: 编译 Rust 后端 ($BUILD_MODE 模式)..."
cd "$PROJECT_DIR"
$CARGO_BUILD_CMD

echo ""
echo "========================================"
echo "  第二部分：构建前端 Electron 应用"
echo "========================================"

echo ""
echo "更新第三方许可证报告..."
if ! cargo about --version >/dev/null 2>&1; then
    echo "cargo-about 未找到，正在安装..."
    cargo install --locked --features cli cargo-about
fi
node "$PROJECT_DIR/scripts/generate_third_party_licenses.mjs"

echo ""
echo "步骤 3/3: 构建并打包 Electron 应用..."
cd "$FRONTEND_DIR"

# 清理旧的前端构建产物
rm -rf "$FRONTEND_DIR/dist"
rm -rf "$FRONTEND_DIR/dist_electron"

# 将后端可执行文件复制到 frontend/resources/backend 目录供 Electron 打包
echo "复制后端可执行文件到 frontend/resources/backend 目录..."
mkdir -p "$FRONTEND_DIR/resources/backend"

echo "收集 Cargo 和 npm 依赖许可证到 frontend/resources/licenses..."
rm -rf "$FRONTEND_DIR/resources/licenses"
mkdir -p "$FRONTEND_DIR/resources/licenses"
node "$PROJECT_DIR/scripts/collect_licenses.mjs" --output "$FRONTEND_DIR/resources/licenses"

if [ -f "$BACKEND_PATH.exe" ]; then
    cp "$BACKEND_PATH.exe" "$FRONTEND_DIR/resources/backend/"
fi
if [ -f "$BACKEND_PATH" ]; then
    cp "$BACKEND_PATH" "$FRONTEND_DIR/resources/backend/"
fi

# 构建 Vue 前端
# npm run build

# 打包 Electron 应用（会自动将 resources 目录打包到最终包中）
npm run electron:build

echo ""
echo "========================================"
echo "  构建完成！"
echo "========================================"
echo ""
if [ "$BUILD_MODE" = "release" ]; then
    ELECTRON_APP_PATH="$FRONTEND_DIR/dist/linux-unpacked/text-search-electron"
else
    ELECTRON_APP_PATH="$FRONTEND_DIR/dist/linux-unpacked/text-search-electron"
fi
echo "输出文件:"
echo "  后端可执行文件：$BACKEND_PATH"
echo "  Electron 应用：  $ELECTRON_APP_PATH"
echo ""
echo "运行方式:"
echo ""
echo "  【方式一】独立运行后端："
echo "    $BACKEND_PATH"
echo ""
echo "  【方式二】开发模式（分别启动前后端）："
echo "    终端 1: cargo run --features with-http-server"
echo "    终端 2: cd frontend && npm run serve"
echo ""
echo "  【方式三】运行 Electron 应用（开发模式）："
echo "    cd frontend && npm run electron:serve"
echo ""
echo "  【方式四】运行打包后的 Electron 应用："
echo "    $ELECTRON_APP_PATH"
echo ""

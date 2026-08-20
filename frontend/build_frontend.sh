#!/bin/bash
# 前端构建脚本：构建 Vue 前端并打包成 Electron 应用
# 注意：请先运行 build_backend.sh 构建后端

set -e

echo "========================================"
echo "  构建前端 Electron 应用"
echo "========================================"

# 获取脚本所在目录
FRONTEND_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$FRONTEND_DIR")"

BUILD_MODE="${1:-dev}"

if [ "$BUILD_MODE" = "release" ]; then
    BACKEND_PATH="$PROJECT_DIR/target/release/text_search"
else
    BACKEND_PATH="$PROJECT_DIR/target/debug/text_search"
fi

# 检查后端是否已构建
if [ ! -f "$BACKEND_PATH" ] && [ ! -f "$PROJECT_DIR/target/release/text_search.exe" ] && [ ! -f "$PROJECT_DIR/target/debug/text_search.exe" ]; then
    echo ""
    echo "警告：未找到后端可执行文件！"
    echo "请先运行：./build_backend.sh"
    echo ""
    echo "是否继续构建前端？(y/n)"
    read -r response
    if [[ "$response" != "y" ]]; then
        exit 1
    fi
fi

echo ""
echo "步骤 1/3: 清理旧的构建产物..."
rm -rf "$FRONTEND_DIR/dist"
rm -rf "$FRONTEND_DIR/dist_electron"

echo ""
echo "步骤 2/3: 构建 Vue 前端..."
cd "$FRONTEND_DIR"
npm run build

echo ""
echo "步骤 3/3: 打包 Electron 应用..."
npm run electron:build

echo ""
echo "========================================"
echo "  前端构建完成！"
echo "========================================"
echo ""
echo "Electron 应用位置：$FRONTEND_DIR/dist_electron/"
echo ""

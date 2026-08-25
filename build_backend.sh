#!/bin/bash
# 后端构建脚本：编译 Rust 后端为独立可执行文件

set -e

echo "========================================"
echo "  构建 Rust 后端"
echo "========================================"

# 获取脚本所在目录作为项目根目录
PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

BUILD_MODE="${1:-dev}"

echo ""
echo "步骤 1/2: 清理旧的构建产物..."
rm -f "$PROJECT_DIR/target/$BUILD_MODE/text_search"
rm -f "$PROJECT_DIR/target/$BUILD_MODE/text_search.exe"

if [ "$BUILD_MODE" = "release" ]; then
    echo ""
    echo "步骤 2/2: 编译 Rust 后端 (release 模式)..."
    cd "$PROJECT_DIR"
    cargo build --release --features with-http-server
    echo ""
    echo "========================================"
    echo "  后端构建完成！"
    echo "========================================"
    echo ""
    echo "可执行文件位置:"
    echo "  Linux/Mac: $PROJECT_DIR/target/release/text_search"
    echo "  Windows:   $PROJECT_DIR\\target\\release\\text_search.exe"
    echo ""
else
    echo ""
    echo "步骤 2/2: 编译 Rust 后端 (dev 模式)..."
    cd "$PROJECT_DIR"
    cargo build --features with-http-server
    echo ""
    echo "========================================"
    echo "  后端构建完成！"
    echo "========================================"
    echo ""
    echo "可执行文件位置:"
    echo "  Linux/Mac: $PROJECT_DIR/target/debug/text_search"
    echo "  Windows:   $PROJECT_DIR\\target\\debug\\text_search.exe"
    echo ""
fi

echo "运行方式:"
if [ "$BUILD_MODE" = "release" ]; then
    echo "  ./target/release/text_search"
else
    echo "  ./target/debug/text_search"
fi
echo ""

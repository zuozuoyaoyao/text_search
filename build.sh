#!/bin/bash
# build.sh - One-click packaging for Linux (Text Search)
#   Modes (arg 1):
#     backend  无 Tauri 版：独立后端，内置 Web UI（浏览器访问）
#     tauri    仅 Tauri 桌面版
#     all      都包含
#   No argument / --help / -h: show this help.
#
# Output: dist/TextSearch-v<ver>-linux-x64[-backend|-tauri].tar.gz

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

usage() {
    cat <<EOF
Usage: $0 [mode]

Modes:
  backend   无 Tauri 版：独立后端，内置 Web UI（浏览器访问）
  tauri     仅 Tauri 桌面版
  all       都包含

Options:
  --help, -h   显示本帮助

Examples:
  $0              # 打印本帮助
  $0 backend      # 仅无 Tauri 版
  $0 tauri        # 仅 Tauri 版
  $0 all          # 都包含

Output: dist/TextSearch-v<ver>-linux-x64[-backend|-tauri].tar.gz
EOF
}

if [ $# -eq 0 ] || [ "$1" = "--help" ] || [ "$1" = "-h" ]; then
    usage
    exit 0
fi

MODE="$1"
case "$MODE" in
  backend|tauri|all) ;;
  *)
    echo "Error: unknown mode '$MODE'"
    echo
    usage
    exit 1
    ;;
esac

echo "========================================"
echo "  Text Search packaging (Linux, mode: $MODE)"
echo "========================================"

step() { echo; echo "==> $1"; }

step "Building frontend (vite)"
npm --prefix "$ROOT/frontend" run build

# frontend/dist is embedded into the backend at compile time via include_dir!.
# Cargo does not track those files, so force a recompile so the binary contains
# the freshly built frontend.
step "Forcing backend recompile to embed latest frontend"
touch "$ROOT/src/lib.rs"

step "Building backend (text_search)"
cargo build --release --features with-ws-server

HOST_TRIPLE="$(rustc -vV | sed -n 's/^host: //p')"
BACKEND_EXE="$ROOT/target/release/text_search"
TAURI_EXE="$ROOT/target/release/text-search-tauri"

if [ "$MODE" = "tauri" ] || [ "$MODE" = "all" ]; then
  step "Copying sidecar -> text_search-$HOST_TRIPLE"
  cp "$BACKEND_EXE" "$ROOT/src-tauri/binaries/text_search-$HOST_TRIPLE"

  step "Building Tauri desktop app"
  npx --prefix "$ROOT/frontend" tauri build --no-bundle
fi

VERSION="$(sed -n 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$ROOT/src-tauri/tauri.conf.json" | head -1)"
SUFFIX=""
[ "$MODE" = "backend" ] && SUFFIX="-backend"
[ "$MODE" = "tauri" ] && SUFFIX="-tauri"
STAGING="$(mktemp -d)"
PKG_NAME="TextSearch-v${VERSION}-linux-x64${SUFFIX}"

step "Staging files"
[ "$MODE" != "tauri" ] && cp "$BACKEND_EXE" "$STAGING/"
[ "$MODE" != "backend" ] && cp "$TAURI_EXE" "$STAGING/"

cat > "$STAGING/README.txt" <<EOF
Text Search v$VERSION (Linux x64, $MODE)
=========================================

Files:
  text_search.exe         Standalone backend with a built-in web UI.
                          Run it, then a browser opens at a random free port
                          (10000-60000, auto-picked).
  text-search-tauri       Desktop app - double click to run.
EOF

mkdir -p "$ROOT/dist"
OUT="$ROOT/dist/$PKG_NAME.tar.gz"
tar czf "$OUT" -C "$STAGING" .
rm -rf "$STAGING"

echo
echo "DONE: $OUT"

#!/bin/bash
# build.sh - One-click packaging for Linux (Text Search)
#   Modes (arg 1):
#     backend  无 Tauri 版：独立后端，内置 Web UI（浏览器访问）
#     tauri    仅 Tauri 桌面版
#     all      都包含
#   No argument / --help / -h: show this help.
#
# Output: dist/TextSearch-v<ver>-linux-x64[-platform][-backend|-tauri].tar.gz
# Optional TEXT_SEARCH_PLATFORM_SUFFIX distinguishes CI platform variants,
# for example -ubuntu22.04 or -ubuntu24.04.

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

Output: dist/TextSearch-v<ver>-linux-x64[-platform][-backend|-tauri].tar.gz
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

echo "==> Updating third-party license report"
if ! cargo about --version >/dev/null 2>&1; then
  echo "cargo-about not found; installing it..."
  cargo install --locked --features cli cargo-about
fi
node "$ROOT/scripts/generate_third_party_licenses.mjs"

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

if [ "$MODE" = "tauri" ] || [ "$MODE" = "all" ]; then
  step "Building backend and preparing Tauri sidecar"
  node "$ROOT/frontend/scripts/build-sidecar.mjs" --release
else
  step "Building backend (text_search)"
  cargo build --release --features with-ws-server
fi

BACKEND_EXE="$ROOT/target/release/text_search"
TAURI_EXE="$ROOT/target/release/text-search-tauri"

if [ "$MODE" = "tauri" ] || [ "$MODE" = "all" ]; then
  step "Building Tauri desktop app"
  npx --prefix "$ROOT/frontend" tauri build --no-bundle
fi

VERSION="$(sed -n 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$ROOT/src-tauri/tauri.conf.json" | head -1)"
PLATFORM_SUFFIX="${TEXT_SEARCH_PLATFORM_SUFFIX:-}"
SUFFIX=""
[ "$MODE" = "backend" ] && SUFFIX="-backend"
[ "$MODE" = "tauri" ] && SUFFIX="-tauri"
STAGING="$(mktemp -d)"
PKG_NAME="TextSearch-v${VERSION}-linux-x64${PLATFORM_SUFFIX}${SUFFIX}"

step "Staging files"
[ "$MODE" != "tauri" ] && cp "$BACKEND_EXE" "$STAGING/"
[ "$MODE" != "backend" ] && cp "$TAURI_EXE" "$STAGING/"
cp "$ROOT/README.md" "$STAGING/README.md"

step "Collecting dependency licenses"
mkdir -p "$STAGING/licenses"
node "$ROOT/scripts/collect_licenses.mjs" --output "$STAGING/licenses"

mkdir -p "$ROOT/dist"
OUT="$ROOT/dist/$PKG_NAME.tar.gz"
tar czf "$OUT" -C "$STAGING" .
rm -rf "$STAGING"

echo
echo "DONE: $OUT"

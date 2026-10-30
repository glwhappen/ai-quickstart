#!/usr/bin/env bash
# 用途：把 Tauri 编出来的可执行文件打成「绿色版」压缩包（不跑安装向导、不进系统目录）
# 用法：./scripts/package-portable.sh [--skip-build]
# 入参：--skip-build  只打包，不重新编译
# 出参：dist-portable/aiquick-<os>-<arch>.zip，并打印体积
# 依赖：pnpm、rust（~/.cargo/env）、zip
# 关键词：portable, 绿色版, 免安装, zip, package, tauri, release, aiquick, 打包, 产物
# 更新：2026-10-30
set -euo pipefail
cd "$(dirname "$0")/.."

OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)
case "$OS" in
  linux) EXT="" ;;
  darwin) EXT="" ;;
  msys*|mingw*|cygwin*) OS=windows; EXT=".exe" ;;
  *) echo "不支持的平台：$OS" >&2; exit 1 ;;
esac

if [ "${1:-}" != "--skip-build" ]; then
  # 必须走 tauri CLI：它会带 `custom-protocol` feature，否则产物会去连 devUrl
  # （direct `cargo build --release` 出来的可执行文件会报 "Could not connect to localhost"）。
  . "$HOME/.cargo/env"
  pnpm tauri build --no-bundle
fi

OUT="dist-portable"
NAME="aiquick-$OS-$ARCH"
rm -rf "$OUT/$NAME" && mkdir -p "$OUT/$NAME"

BIN="src-tauri/target/release/aiquick$EXT"
[ -f "$BIN" ] || { echo "找不到 $BIN，先跑一次构建" >&2; exit 1; }
cp "$BIN" "$OUT/$NAME/"
cp SPEC.md "$OUT/$NAME/README-SPEC.md" 2>/dev/null || true

(cd "$OUT" && rm -f "$NAME.zip" && zip -qr "$NAME.zip" "$NAME")
ls -lh "$OUT/$NAME.zip"

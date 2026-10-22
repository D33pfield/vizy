#!/usr/bin/env bash
# Build for Windows, copy the exe out of WSL, and launch it.
set -euo pipefail
cd "$(dirname "$0")"
profile="${1:-debug}"
flag=""; [[ "$profile" == release ]] && flag="--release"
cargo build $flag
# Your Windows bin folder, or set VIZY_DEST.
win_home=$(wslpath "$(cmd.exe /c 'echo %USERPROFILE%' 2>/dev/null | tr -d '\r')")
dest="${VIZY_DEST:-$win_home/bin}"
mkdir -p "$dest"
cp "target/x86_64-pc-windows-gnu/$profile/vizy.exe" "$dest/vizy.exe"
cd "$dest" && ./vizy.exe

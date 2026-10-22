#!/usr/bin/env bash
# Build for Windows, copy the exe out of WSL, and launch it.
set -euo pipefail
cd "$(dirname "$0")"
profile="${1:-debug}"
flag=""; [[ "$profile" == release ]] && flag="--release"
cargo build $flag
dest=/mnt/c/Users/<you>/bin
mkdir -p "$dest"
cp "target/x86_64-pc-windows-gnu/$profile/vizy.exe" "$dest/vizy.exe"
cd "$dest" && ./vizy.exe

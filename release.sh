#!/usr/bin/env bash
# Build the release .exe and zip it into dist/ for a GitHub release.
#
# Rust bakes source file paths into the binary (for panic messages). They're
# remapped here so the .exe doesn't contain your home directory or username.
set -euo pipefail
cd "$(dirname "$0")"

version=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
name="vizy-$version-windows-x64"
exe=target/x86_64-pc-windows-gnu/release/vizy.exe

# Later flags take priority, so the general home remap comes first.
export RUSTFLAGS="--remap-path-prefix=$HOME=~ \
--remap-path-prefix=$HOME/.cargo/registry/src=crates \
--remap-path-prefix=$PWD=vizy"
cargo build --release

if strings -a "$exe" | grep -qi -e "$HOME" -e "$(whoami)"; then
    echo "error: $exe still contains your home path or username" >&2
    exit 1
fi

mkdir -p dist
python3 - "$name" "$exe" <<'EOF'
import sys, zipfile
name, exe = sys.argv[1], sys.argv[2]
with zipfile.ZipFile(f"dist/{name}.zip", "w", zipfile.ZIP_DEFLATED) as z:
    z.write(exe, f"{name}/vizy.exe")
    z.write("README.md", f"{name}/README.md")
    z.write("UNLICENSE", f"{name}/UNLICENSE")
EOF
echo "dist/$name.zip"

#!/usr/bin/env bash
# Build the `speak` CLI and place it where Tauri expects a sidecar: src-tauri/binaries/speak-<triple>.
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
TRIPLE=$(rustc -vV | sed -n 's/^host: //p')
cd "$ROOT"
cargo build --release -p speechhub-cli
mkdir -p src-tauri/binaries
cp target/release/speak "src-tauri/binaries/speak-$TRIPLE"
echo "speak → src-tauri/binaries/speak-$TRIPLE ($(du -h "src-tauri/binaries/speak-$TRIPLE" | cut -f1))"

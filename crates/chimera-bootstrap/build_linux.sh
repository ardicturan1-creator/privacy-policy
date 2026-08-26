#!/usr/bin/env bash
# CHIMERA / MONOLITH -- Linux/macOS derleme betigi.
#
# Kaynaktan gercek bir binary uretir. Gereksinim: Rust (https://rustup.rs).
# Baska hicbir sistem paketi (Docker, Python, CUDA toolkit vb.) gerekmez.
set -euo pipefail
cd "$(dirname "$0")"

if ! command -v cargo >/dev/null 2>&1; then
    echo "[HATA] Rust/cargo bulunamadi. Once https://rustup.rs adresinden kurun." >&2
    exit 1
fi

echo "[1/2] cargo test  --  once GERCEK test paketi calistirilir"
cargo test --release

echo "[2/2] cargo build --release"
cargo build --release

echo
echo "Basarili: target/release/chimera-bootstrap"
echo
echo "Deneyin:"
echo "  target/release/chimera-bootstrap probe"
echo "  target/release/chimera-bootstrap install --root /tmp/chimera"
echo "  target/release/chimera-bootstrap verify   --root /tmp/chimera"
echo "  target/release/chimera-bootstrap obsidian-demo"

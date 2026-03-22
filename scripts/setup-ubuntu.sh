#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "[1/4] Installing Ubuntu packages for Tauri..."
sudo apt-get update
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  patchelf \
  libxdo-dev \
  libssl-dev

if ! command -v rustup >/dev/null 2>&1; then
  echo "[2/4] Installing rustup and Rust stable..."
  curl https://sh.rustup.rs -sSf | sh -s -- -y --profile default
else
  echo "[2/4] rustup already exists. Updating stable toolchain..."
fi

export PATH="${HOME}/.cargo/bin:${PATH}"
rustup default stable

echo "[3/4] Installing npm dependencies..."
cd "${ROOT_DIR}"
npm install

echo "[4/4] Toolchain summary"
node -v
npm -v
rustc -V
cargo -V

echo
echo "Setup complete."
echo "Run 'npm run tauri:dev' from ${ROOT_DIR} to start the desktop app."

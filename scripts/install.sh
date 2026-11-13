#!/usr/bin/env bash
set -euo pipefail
OGH_SOURCE_DIR="$(cd "$(dirname "$0")/.." && pwd)"
npm --prefix "$OGH_SOURCE_DIR/web" ci
npm --prefix "$OGH_SOURCE_DIR/web" run build
cargo install --path "$OGH_SOURCE_DIR" --locked
OGH_EXECUTABLE="${CARGO_HOME:-$HOME/.cargo}/bin/ogh"
"$OGH_EXECUTABLE" app install --web-dir "$OGH_SOURCE_DIR/web/dist"
mkdir -p "$HOME/.local/bin"
if [ -d "$HOME/.local/bin/ogh" ]; then
  echo 'Cannot install CLI link: ~/.local/bin/ogh is a directory' >&2
  exit 1
fi
if [ -e "$HOME/.local/bin/ogh" ] && [ ! -L "$HOME/.local/bin/ogh" ]; then
  OGH_PREVIOUS_BINARY="$(mktemp "$HOME/.local/bin/ogh.backup.XXXXXX")"
  mv "$HOME/.local/bin/ogh" "$OGH_PREVIOUS_BINARY"
  echo "Previous CLI retained at $OGH_PREVIOUS_BINARY"
fi
ln -sfn "$OGH_EXECUTABLE" "$HOME/.local/bin/ogh"
echo 'Installed. Ensure ~/.local/bin or ~/.cargo/bin is on PATH, then run: ogh serve'

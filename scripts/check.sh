#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --locked
cargo build --locked
python3 scripts/smoke.py
python3 scripts/test_outbox.py
python3 scripts/test_cli.py
python3 scripts/test_setup.py
npm --prefix web ci
npm --prefix web run build

if [ "${OGH_BROWSER_TESTS:-0}" = "1" ]; then
  web/node_modules/.bin/playwright install --with-deps chromium
  python3 scripts/test_browser.py
fi

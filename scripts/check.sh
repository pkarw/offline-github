#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo fmt --check
cargo clippy --all-targets -- -D warnings
# Subprocess tests fork; serialize to avoid inheriting another test's flock FD.
cargo test --locked -- --test-threads=1
cargo build --locked
# Server integration tests require built frontend assets on clean checkouts.
npm --prefix web ci
npm --prefix web run build
node scripts/test_local_items.cjs
python3 scripts/smoke.py
python3 scripts/test_outbox.py
python3 scripts/test_cli.py
python3 scripts/test_setup.py
python3 scripts/test_launcher.py

if [ "${OGH_BROWSER_TESTS:-0}" = "1" ]; then
  web/node_modules/.bin/playwright install --with-deps chromium
  python3 scripts/test_browser.py
fi

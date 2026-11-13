#!/usr/bin/env bash
# Downloadable entry point. bash <(curl ...) keeps stdin free for login and the shell.
set -euo pipefail
OGH_SETUP_REF="${OGH_SETUP_REF:-main}"
OGH_SETUP_ROOT="${OGH_SETUP_ROOT:-$HOME/.local/share/ogh-setup}"
OGH_WORKSPACE="${OGH_WORKSPACE:-$HOME/Projects/open-mercato-offline}"
OGH_SETUP_PORT="${OGH_SETUP_PORT:-4173}"
OGH_SETUP_SHELL=1
OGH_SETUP_SYNC=1
OGH_SETUP_OPEN=1
while [ $# -gt 0 ]; do
  case "$1" in
    --no-shell) OGH_SETUP_SHELL=0 ;;
    --skip-sync) OGH_SETUP_SYNC=0 ;;
    --no-open) OGH_SETUP_OPEN=0 ;;
    --help) echo 'Usage: bash setup.sh [--no-shell] [--no-open] [--skip-sync]'; exit 0 ;;
    *) echo "Unknown option: $1" >&2; exit 1 ;;
  esac
  shift
done
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:/opt/homebrew/bin:$PATH"
mkdir -p "$OGH_SETUP_ROOT" "$HOME/.local/bin"
# Install prerequisites using the host package manager, never an implicit sudo.
missing=()
for tool in git gh jq python3; do command -v "$tool" >/dev/null || missing+=("$tool"); done
if [ ${#missing[@]} -gt 0 ]; then
  if command -v brew >/dev/null; then
    brew install "${missing[@]}"
  elif command -v apt-get >/dev/null && [ "$(id -u)" = 0 ]; then
    apt-get update && apt-get install -y git gh jq python3 curl ca-certificates build-essential pkg-config libssl-dev
  else
    echo "Install these prerequisites first using your package manager: ${missing[*]}" >&2
    echo 'Debian/Ubuntu: sudo apt-get install git gh jq python3 curl build-essential pkg-config libssl-dev' >&2
    echo 'macOS: install Homebrew from https://brew.sh, then rerun.' >&2
    exit 1
  fi
fi
if ! command -v node >/dev/null || [ "$(node -p 'Number(process.versions.node.split(".")[0]) >= 22')" != true ]; then
  if command -v brew >/dev/null; then
    brew install node@22
    export PATH="$(brew --prefix node@22)/bin:$PATH"
  else
    echo 'Node.js 22+ is required. Install it, then rerun (sandbox image already includes it).' >&2
    exit 1
  fi
fi
source_dir="${OGH_SETUP_SOURCE:-$OGH_SETUP_ROOT/offline-github}"
if [ -n "${OGH_SETUP_SOURCE:-}" ]; then
  [ -f "$source_dir/scripts/setup/configure.py" ] || { echo "Invalid OGH_SETUP_SOURCE" >&2; exit 1; }
elif [ -d "$source_dir/.git" ]; then
  [ -z "$(git -C "$source_dir" status --porcelain)" ] || { echo "Preserving modified installer checkout: $source_dir" >&2; exit 1; }
  git -C "$source_dir" fetch origin "$OGH_SETUP_REF"
  git -C "$source_dir" checkout --detach FETCH_HEAD
else
  [ ! -e "$source_dir" ] || { echo "Destination exists: $source_dir" >&2; exit 1; }
  git clone --depth 1 --branch "$OGH_SETUP_REF" https://github.com/pkarw/offline-github.git "$source_dir"
fi
# Apple Silicon users need no Rust build. Validate the release checksum before installation.
if [ "$(uname -s)-$(uname -m)" = Darwin-arm64 ]; then
  bundle="$(mktemp -d "$OGH_SETUP_ROOT/bundle.XXXXXX")"
  curl --fail --location --retry 3 -o "$bundle/app.tgz" https://github.com/pkarw/offline-github/releases/download/v0.2.0/offline-github-macos-arm64.tgz
  curl --fail --location --retry 3 -o "$bundle/SHA256SUMS.txt" https://github.com/pkarw/offline-github/releases/download/v0.2.0/SHA256SUMS.txt
  expected="$(awk '$2 == "offline-github-macos-arm64.tgz" {print $1}' "$bundle/SHA256SUMS.txt")"
  actual="$(shasum -a 256 "$bundle/app.tgz" | awk '{print $1}')"
  [ -n "$expected" ] && [ "$actual" = "$expected" ] || { echo 'Bundle checksum mismatch' >&2; exit 1; }
  tar -xzf "$bundle/app.tgz" -C "$bundle"
  # The user's existing ~/.local/bin/ogh may be a symlink into Cargo.
  if [ -e "$HOME/.local/bin/ogh" ] || [ -L "$HOME/.local/bin/ogh" ]; then
    cp -L "$HOME/.local/bin/ogh" "$bundle/previous-ogh"
    rm "$HOME/.local/bin/ogh"
  fi
  install -m 755 "$bundle/ogh" "$HOME/.local/bin/ogh"
  ogh app install --web-dir "$bundle/web"
else
  if ! command -v cargo >/dev/null; then
    curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs -o "$OGH_SETUP_ROOT/rustup.sh"
    sh "$OGH_SETUP_ROOT/rustup.sh" -y --profile minimal
    export PATH="$HOME/.cargo/bin:$PATH"
  fi
  bash "$source_dir/scripts/install.sh"
fi
# Authentication is interactive and stays in the user's normal gh credential store.
if ! gh auth status >/dev/null 2>&1; then
  gh auth login --hostname github.com --git-protocol https --web < /dev/tty
fi
repo_record="$(ogh repo add open-mercato/open-mercato)"
mirror="$(printf '%s' "$repo_record" | jq -r .path)"
if [ "$OGH_SETUP_SYNC" = 1 ]; then
  echo 'Downloading repository history and all issues/PRs; the first sync can take several minutes.'
  ogh sync -R open-mercato/open-mercato
fi
if [ -e "$OGH_WORKSPACE" ]; then
  [ -f "$OGH_WORKSPACE/.ogh-workspace.json" ] || { echo "Preserving existing folder. Choose OGH_WORKSPACE=/new/path: $OGH_WORKSPACE" >&2; exit 1; }
  git -C "$OGH_WORKSPACE" rev-parse --git-dir >/dev/null
else
  base="$(ogh repo view -R open-mercato/open-mercato --json defaultBranchRef --jq .defaultBranchRef.name)"
  mkdir -p "$(dirname "$OGH_WORKSPACE")"
  git --git-dir="$mirror" worktree add --detach "$OGH_WORKSPACE" "origin/$base"
  git -C "$OGH_WORKSPACE" switch -c "offline/work-$(date +%Y%m%d-%H%M%S)-$$"
fi
skills_dir="$OGH_SETUP_ROOT/skills"
# Keep exact source provenance; never overwrite modified or already installed skills.
if [ ! -d "$skills_dir/.git" ]; then
  git clone --depth 1 https://github.com/open-mercato/skills.git "$skills_dir"
fi
python3 "$source_dir/scripts/setup/configure.py" "$OGH_WORKSPACE" "$skills_dir" "$source_dir"
# Helper enters the project, exports PATH/repo, then starts an interactive user shell.
python3 "$source_dir/scripts/setup/finish.py" "$OGH_WORKSPACE" "$OGH_SETUP_ROOT" "$OGH_SETUP_PORT"
if [ "$OGH_SETUP_OPEN" = 1 ] && command -v open >/dev/null; then open "http://127.0.0.1:$OGH_SETUP_PORT"; fi
printf '\nReady. App: http://127.0.0.1:%s\nProject: %s\n' "$OGH_SETUP_PORT" "$OGH_WORKSPACE"
echo 'Start pi, codex or claude here. Project skills use ogh; gh remains available for explicit online sync.'
echo 'Open Mercato application dependencies/services are separate; see its README when running the ERP itself.'
if [ "$OGH_SETUP_SHELL" = 1 ]; then
  exec bash "$HOME/.local/bin/ogh-work"
fi

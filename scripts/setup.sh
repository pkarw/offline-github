#!/usr/bin/env bash
# Downloadable entry point. bash <(curl ...) keeps stdin free for login and the shell.
set -euo pipefail
OGH_SETUP_REF="${OGH_SETUP_REF:-main}"
OGH_SETUP_ROOT="${OGH_SETUP_ROOT:-$HOME/.offline-github/setup}"
workspace_supplied="${OGH_WORKSPACE:-}"
OGH_WORKSPACE="${OGH_WORKSPACE:-$HOME/Projects/open-mercato-offline}"
OGH_SETUP_PORT="${OGH_SETUP_PORT:-4173}"
OGH_SETUP_SHELL=1
OGH_SETUP_SYNC=1
OGH_SETUP_OPEN=1
OGH_SETUP_REPO="${OGH_SETUP_REPO:-open-mercato/open-mercato}"
OGH_REPO_PROMPT=auto
OGH_SETUP_PROMPT=auto
OGH_SETUP_PLAN=0
while [ $# -gt 0 ]; do
  case "$1" in
    --folder|-d) [ $# -ge 2 ] && [ -n "$2" ] && [[ "$2" != --* ]] || { echo 'Missing --folder value' >&2; exit 1; }; OGH_WORKSPACE="$2"; workspace_supplied="$2"; OGH_SETUP_PROMPT=off; shift ;;
    --repo|-R) [ $# -ge 2 ] && [ -n "$2" ] && [[ "$2" != --* ]] || { echo 'Missing --repo value' >&2; exit 1; }; OGH_SETUP_REPO="$2"; OGH_REPO_PROMPT=off; shift ;;
    --no-prompt) OGH_SETUP_PROMPT=off; OGH_REPO_PROMPT=off ;;
    --interactive) OGH_SETUP_PROMPT=on; OGH_REPO_PROMPT=on ;;
    --plan) OGH_SETUP_PLAN=1 ;;
    --no-shell) OGH_SETUP_SHELL=0 ;;
    --skip-sync) OGH_SETUP_SYNC=0 ;;
    --no-open) OGH_SETUP_OPEN=0 ;;
    --help) printf '%s\n' 'Usage: bash setup.sh [--repo OWNER/REPO] [--folder NAME|PATH] [--interactive|--no-prompt] [--plan]' '                     [--no-shell] [--no-open] [--skip-sync]' 'Bare folder names go under ~/Projects; paths containing / are relative to your current folder.' 'Repository defaults to open-mercato/open-mercato; GitHub HTTPS URLs are also accepted.' 'Interactive terminals prompt for the repository and folder. --plan only shows destinations; installs nothing.'; exit 0 ;;
    *) echo "Unknown option: $1" >&2; exit 1 ;;
  esac
  shift
done
# Show the destination and let the user change it before any installation work.
printf '\n🛫 Offline GitHub · local setup\n'
printf '🐙 GitHub repository: %s\n' "$OGH_SETUP_REPO"
if [ "$OGH_REPO_PROMPT" = on ] || { [ "$OGH_REPO_PROMPT" = auto ] && [ -t 0 ] && [ "$OGH_SETUP_PLAN" = 0 ]; }; then
  printf '✏️  GitHub repository (Enter keeps the value above): '
  IFS= read -r chosen_repo || { echo 'Setup cancelled before installation.' >&2; exit 1; }
  if [ -n "$chosen_repo" ]; then OGH_SETUP_REPO="$chosen_repo"; fi
fi
OGH_SETUP_REPO="${OGH_SETUP_REPO#https://github.com/}"
OGH_SETUP_REPO="${OGH_SETUP_REPO%/}"
OGH_SETUP_REPO="${OGH_SETUP_REPO%.git}"
[[ "$OGH_SETUP_REPO" =~ ^[a-zA-Z0-9][a-zA-Z0-9-]*/[a-zA-Z0-9_.-]+$ ]] && [[ "${OGH_SETUP_REPO#*/}" != . ]] && [[ "${OGH_SETUP_REPO#*/}" != .. ]] || { echo 'Invalid repository; use OWNER/REPO or https://github.com/OWNER/REPO.' >&2; exit 1; }
printf '📦 Selected repository: %s\n' "$OGH_SETUP_REPO"
if [ -z "$workspace_supplied" ]; then OGH_WORKSPACE="$HOME/Projects/${OGH_SETUP_REPO#*/}-offline"; fi
printf '📂 Project folder: %s\n' "$OGH_WORKSPACE"
if [ "$OGH_SETUP_PROMPT" = on ] || { [ "$OGH_SETUP_PROMPT" = auto ] && [ -t 0 ] && [ "$OGH_SETUP_PLAN" = 0 ]; }; then
  printf '✏️  Project folder (Enter keeps the default): '
  IFS= read -r chosen_folder || { echo 'Setup cancelled before installation.' >&2; exit 1; }
  if [ -n "$chosen_folder" ]; then OGH_WORKSPACE="$chosen_folder"; fi
fi
case "$OGH_WORKSPACE" in
  '~/'*) OGH_WORKSPACE="$HOME/${OGH_WORKSPACE#\~/}" ;;
  /*) ;;
  */*) OGH_WORKSPACE="$PWD/$OGH_WORKSPACE" ;;
  *) OGH_WORKSPACE="$HOME/Projects/$OGH_WORKSPACE" ;;
esac
printf '📂 Install project and skills: %s\n' "$OGH_WORKSPACE"
printf '🧰 Installer files: %s\n' "$OGH_SETUP_ROOT"
printf '🔧 CLI: %s/.local/bin/ogh · reentry: %s/.local/bin/ogh-work\n' "$HOME" "$HOME"
display_cache="$HOME/.offline-github"
printf '💾 App cache and UI: %s\n' "${OGH_DATA_DIR:-$display_cache}"
printf '🌐 Browser app: http://127.0.0.1:%s\n\n' "$OGH_SETUP_PORT"
if [ "$OGH_SETUP_PLAN" = 1 ]; then exit 0; fi
# Preserve live legacy caches and their Git worktree paths through a compatibility link.
if [ -z "${OGH_DATA_DIR:-}" ] && [ ! -e "$HOME/.offline-github" ] && [ ! -L "$HOME/.offline-github" ]; then
  case "$(uname -s)" in
    Darwin) legacy_cache="$HOME/Library/Application Support/offline-github" ;;
    *) legacy_cache="${XDG_DATA_HOME:-$HOME/.local/share}/offline-github" ;;
  esac
  if [ -d "$legacy_cache" ]; then ln -s "$legacy_cache" "$HOME/.offline-github"; fi
fi
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
  curl --fail --location --retry 3 -o "$bundle/app.tgz" https://github.com/pkarw/offline-github/releases/download/v0.2.6/offline-github-macos-arm64.tgz
  curl --fail --location --retry 3 -o "$bundle/SHA256SUMS.txt" https://github.com/pkarw/offline-github/releases/download/v0.2.6/SHA256SUMS.txt
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
repo_record="$(ogh repo add "$OGH_SETUP_REPO")"
mirror="$(printf '%s' "$repo_record" | jq -r .path)"
if [ "$OGH_SETUP_SYNC" = 1 ]; then
  echo '📥 Downloading repository history and all issues/PRs; the first sync can take several minutes.'
  ogh sync -R "$OGH_SETUP_REPO"
fi
if [ -e "$OGH_WORKSPACE" ]; then
  [ -f "$OGH_WORKSPACE/.ogh-workspace.json" ] || { echo "Preserving existing folder. Choose OGH_WORKSPACE=/new/path: $OGH_WORKSPACE" >&2; exit 1; }
  python3 - "$OGH_WORKSPACE/.ogh-workspace.json" "$OGH_SETUP_REPO" <<'PYREPO'
import json, sys
actual = json.load(open(sys.argv[1]))['repository']
if actual.lower() != sys.argv[2].lower():
    raise SystemExit(f'Preserving workspace for {actual}; choose a different --folder for {sys.argv[2]}.')
PYREPO
  git -C "$OGH_WORKSPACE" rev-parse --git-dir >/dev/null
else
  base="$(ogh repo view -R "$OGH_SETUP_REPO" --json defaultBranchRef --jq .defaultBranchRef.name)"
  mkdir -p "$(dirname "$OGH_WORKSPACE")"
  python3 "$source_dir/scripts/setup/worktree.py" "$mirror" "$OGH_WORKSPACE" "origin/$base"
  git -C "$OGH_WORKSPACE" switch -c "offline/work-$(date +%Y%m%d-%H%M%S)-$$"
fi
skills_dir="$OGH_SETUP_ROOT/skills"
# Keep exact source provenance; never overwrite modified or already installed skills.
if [ ! -d "$skills_dir/.git" ]; then
  git clone --depth 1 https://github.com/open-mercato/skills.git "$skills_dir"
fi
echo '🤖 Installing project skills for ogh…'
python3 "$source_dir/scripts/setup/configure.py" "$OGH_WORKSPACE" "$skills_dir" "$source_dir" --repo "$OGH_SETUP_REPO"
# Helper enters the project, exports PATH/repo, then starts an interactive user shell.
python3 "$source_dir/scripts/setup/finish.py" "$OGH_WORKSPACE" "$OGH_SETUP_ROOT" "$OGH_SETUP_PORT" "$OGH_SETUP_OPEN"
app_url="$(cat "$OGH_SETUP_ROOT/server.url")"

printf '\n✅ Ready! App: %s\nProject: %s\n' "$app_url" "$OGH_WORKSPACE"
echo 'Start pi, codex or claude here. Project skills use ogh; gh remains available for explicit online sync.'
echo 'Project application dependencies/services are separate; follow its README to run the application.'
if [ "$OGH_SETUP_SHELL" = 1 ]; then
  exec bash "$HOME/.local/bin/ogh-work"
fi

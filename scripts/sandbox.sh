#!/usr/bin/env bash
# Persistent local microVM. No host home, Git credentials or agent tokens mounted.
set -euo pipefail
export PATH="$HOME/.microsandbox/bin:$HOME/.local/bin:/opt/homebrew/bin:$PATH"
OGH_SANDBOX_NAME="${OGH_SANDBOX_NAME:-ogh-mercato}"
port_supplied="${OGH_SANDBOX_PORT:-}"
OGH_SANDBOX_PORT="${OGH_SANDBOX_PORT:-4183}"
OGH_SETUP_REF="${OGH_SETUP_REF:-main}"
folder_supplied="${OGH_SANDBOX_FOLDER:-}"
OGH_SANDBOX_FOLDER="${OGH_SANDBOX_FOLDER:-open-mercato-offline}"
OGH_SETUP_REPO="${OGH_SETUP_REPO:-open-mercato/open-mercato}"
OGH_REPO_PROMPT=auto
OGH_SETUP_PROMPT=auto
OGH_SETUP_PLAN=0
while [ $# -gt 0 ]; do
  case "$1" in
    --folder|-d) [ $# -ge 2 ] && [ -n "$2" ] && [[ "$2" != --* ]] || { echo 'Missing --folder value' >&2; exit 1; }; OGH_SANDBOX_FOLDER="$2"; folder_supplied="$2"; OGH_SETUP_PROMPT=off; shift ;;
    --repo|-R) [ $# -ge 2 ] && [ -n "$2" ] && [[ "$2" != --* ]] || { echo 'Missing --repo value' >&2; exit 1; }; OGH_SETUP_REPO="$2"; OGH_REPO_PROMPT=off; shift ;;
    --no-prompt) OGH_SETUP_PROMPT=off; OGH_REPO_PROMPT=off ;;
    --interactive) OGH_SETUP_PROMPT=on; OGH_REPO_PROMPT=on ;;
    --plan) OGH_SETUP_PLAN=1 ;;
    --help) printf '%s\n' 'Usage: bash sandbox.sh [--repo OWNER/REPO] [--folder NAME] [--interactive|--no-prompt] [--plan]' 'Folder name is inside the guest at /root/Projects/NAME (letters, digits, spaces, dots, dashes, underscores).' 'Repository defaults to open-mercato/open-mercato; GitHub HTTPS URLs are also accepted.' 'Interactive terminals prompt for the repository and folder. --plan only shows destinations; installs nothing.'; exit 0 ;;
    *) echo "Unknown option: $1" >&2; exit 1 ;;
  esac
  shift
done
printf '\n🧊 Offline GitHub · Microsandbox setup\n'
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
if [ -z "$folder_supplied" ]; then OGH_SANDBOX_FOLDER="${OGH_SETUP_REPO#*/}-offline"; fi
printf '📂 Project folder inside VM: /root/Projects/%s\n' "$OGH_SANDBOX_FOLDER"
if [ "$OGH_SETUP_PROMPT" = on ] || { [ "$OGH_SETUP_PROMPT" = auto ] && [ -t 0 ] && [ "$OGH_SETUP_PLAN" = 0 ]; }; then
  printf '✏️  Guest folder name (Enter keeps the default): '
  IFS= read -r chosen_folder || { echo 'Setup cancelled before installation.' >&2; exit 1; }
  if [ -n "$chosen_folder" ]; then OGH_SANDBOX_FOLDER="$chosen_folder"; fi
fi
[[ "$OGH_SANDBOX_FOLDER" =~ ^[a-zA-Z0-9][a-zA-Z0-9._\ -]*$ ]] || { echo 'Invalid guest folder name; use letters, digits, spaces, dots, dashes or underscores.' >&2; exit 1; }
OGH_GUEST_WORKSPACE="/root/Projects/$OGH_SANDBOX_FOLDER"
[[ "$OGH_SANDBOX_NAME" =~ ^[a-zA-Z0-9_-]+$ ]] || { echo 'Invalid sandbox name' >&2; exit 1; }
[[ "$OGH_SANDBOX_PORT" =~ ^[0-9]+$ ]] && [ "$OGH_SANDBOX_PORT" -ge 1 ] && [ "$OGH_SANDBOX_PORT" -le 65535 ] || { echo 'Invalid port' >&2; exit 1; }
[[ "$OGH_SETUP_REF" =~ ^[a-zA-Z0-9._/-]+$ ]] || { echo 'Invalid source ref' >&2; exit 1; }
printf '📂 Install project and skills: %s (inside VM %s)\n' "$OGH_GUEST_WORKSPACE" "$OGH_SANDBOX_NAME"
printf '💾 Persistent guest disk: %s-home → /root\n' "$OGH_SANDBOX_NAME"
printf '🧰 Guest tools/cache: /root/.local/share · CLI: /root/.local/bin/ogh\n'
printf '🔧 Microsandbox on host: existing msb or %s/.microsandbox/bin/msb\n' "$HOME"
printf '🔑 Reentry on host: %s/.local/bin/ogh-sandbox\n' "$HOME"
printf '🌐 Browser app on host: http://127.0.0.1:%s\n' "$OGH_SANDBOX_PORT"
printf '🔒 Your host home and credentials stay outside the VM.\n\n'
if [ "$OGH_SETUP_PLAN" = 1 ]; then exit 0; fi
if ! command -v msb >/dev/null; then
  installer="$(mktemp)"
  curl -fsSL https://install.microsandbox.dev -o "$installer"
  sh "$installer"
  rm "$installer"
fi
echo '🔍 Checking Microsandbox…'
msb doctor
state_dir="$HOME/.local/share/ogh-sandbox"
mkdir -p "$state_dir"
receipt="$state_dir/$OGH_SANDBOX_NAME.port"
# inspect is local metadata; never replace or remove an existing VM/volume.
if msb inspect "$OGH_SANDBOX_NAME" >/dev/null 2>&1; then
  if [ -z "$port_supplied" ] && [ -f "$receipt" ]; then OGH_SANDBOX_PORT="$(cat "$receipt")"; fi
  [ -f "$receipt" ] && [ "$(cat "$receipt")" = "$OGH_SANDBOX_PORT" ] || { echo "Preserving existing VM or port mapping. Choose a new OGH_SANDBOX_NAME." >&2; exit 1; }
  msb ping "$OGH_SANDBOX_NAME" >/dev/null 2>&1 || msb start "$OGH_SANDBOX_NAME"
else
  OGH_SANDBOX_PORT="$(python3 - "$OGH_SANDBOX_PORT" <<'PYPORT'
import socket, sys
with socket.socket() as sock:
    try: sock.bind(('127.0.0.1', int(sys.argv[1])))
    except OSError: sock.bind(('127.0.0.1', 0))
    print(sock.getsockname()[1])
PYPORT
)"
  echo "🌐 Using host URL http://127.0.0.1:$OGH_SANDBOX_PORT"
  echo '📦 Creating your persistent development VM…'
  msb create --name "$OGH_SANDBOX_NAME" --cpus 4 --memory 8G --root-disk 32G \
    --mount-named "$OGH_SANDBOX_NAME-home:/root:kind=disk,size=40G" \
    --port "127.0.0.1:$OGH_SANDBOX_PORT:4173" \
    --workdir /root --user root docker.io/library/node:22-bookworm
  printf '%s\n' "$OGH_SANDBOX_PORT" > "$receipt"
fi
# Bootstrap is separate from shell attachment so interruption never destroys local work.
msb exec --timeout 60m "$OGH_SANDBOX_NAME" -- bash -c '
  set -euo pipefail
  apt-get update
  apt-get install -y gh jq python3 curl ca-certificates build-essential pkg-config libssl-dev socat
  export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
  mkdir -p /root/.local/share/ogh-setup
  curl -fsSL "https://raw.githubusercontent.com/pkarw/offline-github/'"$OGH_SETUP_REF"'/scripts/setup.sh" -o /root/.local/share/ogh-setup/bootstrap.sh
'
# Install/login remains interactive; GitHub credentials are entered inside the guest only.
msb exec --tty --env "OGH_SETUP_REF=$OGH_SETUP_REF" --env OGH_SETUP_PORT=4174 "$OGH_SANDBOX_NAME" -- \
  bash /root/.local/share/ogh-setup/bootstrap.sh --repo "$OGH_SETUP_REPO" --folder "$OGH_GUEST_WORKSPACE" --no-prompt --no-shell --no-open
# Raw TCP relay preserves localhost Host/Origin checks from the browser.
msb exec --detach "$OGH_SANDBOX_NAME" -- bash -c 'pgrep -x socat >/dev/null || exec socat TCP-LISTEN:4173,bind=0.0.0.0,reuseaddr,fork TCP:127.0.0.1:$(cat /root/.local/share/ogh-setup/server.url | sed "s/.*://")'
mkdir -p "$HOME/.local/bin"
launcher="$HOME/.local/bin/ogh-sandbox"
# Values have been validated before embedding in shell source.
temporary="$(mktemp "$HOME/.local/bin/ogh-sandbox.XXXXXX")"
cat > "$temporary" <<EOF
#!/usr/bin/env bash
set -euo pipefail
export PATH="\$HOME/.microsandbox/bin:/opt/homebrew/bin:\$PATH"
msb ping '$OGH_SANDBOX_NAME' >/dev/null 2>&1 || msb start '$OGH_SANDBOX_NAME'
msb exec --tty --env 'OGH_WORKSPACE=$OGH_GUEST_WORKSPACE' '$OGH_SANDBOX_NAME' -- bash -c 'export PATH="\$HOME/.local/bin:\$HOME/.cargo/bin:\$PATH"; python3 /root/.local/share/ogh-setup/offline-github/scripts/setup/finish.py "\$OGH_WORKSPACE" /root/.local/share/ogh-setup 4174'
msb exec --detach '$OGH_SANDBOX_NAME' -- bash -c 'pgrep -x socat >/dev/null || exec socat TCP-LISTEN:4173,bind=0.0.0.0,reuseaddr,fork TCP:127.0.0.1:\$(cat /root/.local/share/ogh-setup/server.url | sed "s/.*://")'
exec msb exec --tty '$OGH_SANDBOX_NAME' -- bash /root/.local/bin/ogh-work
EOF
chmod +x "$temporary"
if [ -e "$launcher" ] || [ -L "$launcher" ]; then
  [ ! -d "$launcher" ] || [ -L "$launcher" ] || { echo "Preserving helper directory: $launcher" >&2; exit 1; }
  backup="$(mktemp -d "$HOME/.local/bin/ogh-sandbox.backup.XXXXXX")"
  mv "$launcher" "$backup/ogh-sandbox"
  echo "Previous helper retained at $backup/ogh-sandbox"
fi
mv "$temporary" "$launcher"
echo "✅ Ready: http://127.0.0.1:$OGH_SANDBOX_PORT — next time run ~/.local/bin/ogh-sandbox"
if command -v open >/dev/null; then open "http://127.0.0.1:$OGH_SANDBOX_PORT" || true
elif command -v xdg-open >/dev/null; then xdg-open "http://127.0.0.1:$OGH_SANDBOX_PORT" || true; fi
exec msb exec --tty "$OGH_SANDBOX_NAME" -- bash /root/.local/bin/ogh-work

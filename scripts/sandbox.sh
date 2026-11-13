#!/usr/bin/env bash
# Persistent local microVM. No host home, Git credentials or agent tokens mounted.
set -euo pipefail
export PATH="$HOME/.microsandbox/bin:$HOME/.local/bin:/opt/homebrew/bin:$PATH"
OGH_SANDBOX_NAME="${OGH_SANDBOX_NAME:-ogh-mercato}"
OGH_SANDBOX_PORT="${OGH_SANDBOX_PORT:-4183}"
OGH_SETUP_REF="${OGH_SETUP_REF:-main}"
[[ "$OGH_SANDBOX_NAME" =~ ^[a-zA-Z0-9_-]+$ ]] || { echo 'Invalid sandbox name' >&2; exit 1; }
[[ "$OGH_SANDBOX_PORT" =~ ^[0-9]+$ ]] && [ "$OGH_SANDBOX_PORT" -ge 1 ] && [ "$OGH_SANDBOX_PORT" -le 65535 ] || { echo 'Invalid port' >&2; exit 1; }
[[ "$OGH_SETUP_REF" =~ ^[a-zA-Z0-9._/-]+$ ]] || { echo 'Invalid source ref' >&2; exit 1; }
if ! command -v msb >/dev/null; then
  installer="$(mktemp)"
  curl -fsSL https://install.microsandbox.dev -o "$installer"
  sh "$installer"
  rm "$installer"
fi
msb doctor
# inspect is local metadata; never replace or remove an existing VM/volume.
if msb inspect "$OGH_SANDBOX_NAME" >/dev/null 2>&1; then
  msb ping "$OGH_SANDBOX_NAME" >/dev/null 2>&1 || msb start "$OGH_SANDBOX_NAME"
else
  msb create --name "$OGH_SANDBOX_NAME" --cpus 4 --memory 8G --root-disk 32G \
    --mount-named "$OGH_SANDBOX_NAME-home:/root:kind=disk,size=40G" \
    --port "127.0.0.1:$OGH_SANDBOX_PORT:4173" \
    --workdir /root --user root docker.io/library/node:22-bookworm
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
  bash /root/.local/share/ogh-setup/bootstrap.sh --no-shell --no-open
# Raw TCP relay preserves localhost Host/Origin checks from the browser.
msb exec --detach "$OGH_SANDBOX_NAME" -- socat TCP-LISTEN:4173,bind=0.0.0.0,reuseaddr,fork TCP:127.0.0.1:4174
mkdir -p "$HOME/.local/bin"
launcher="$HOME/.local/bin/ogh-sandbox"
# Values have been validated before embedding in shell source.
cat > "$launcher" <<EOF
#!/usr/bin/env bash
set -euo pipefail
export PATH="\$HOME/.microsandbox/bin:/opt/homebrew/bin:\$PATH"
msb ping '$OGH_SANDBOX_NAME' >/dev/null 2>&1 || msb start '$OGH_SANDBOX_NAME'
msb exec --tty '$OGH_SANDBOX_NAME' -- bash -c 'export PATH="\$HOME/.local/bin:\$HOME/.cargo/bin:\$PATH"; python3 /root/.local/share/ogh-setup/offline-github/scripts/setup/finish.py /root/Projects/open-mercato-offline /root/.local/share/ogh-setup 4174'
msb exec --detach '$OGH_SANDBOX_NAME' -- bash -c 'pgrep -x socat >/dev/null || exec socat TCP-LISTEN:4173,bind=0.0.0.0,reuseaddr,fork TCP:127.0.0.1:4174'
exec msb exec --tty '$OGH_SANDBOX_NAME' -- bash /root/.local/bin/ogh-work
EOF
chmod +x "$launcher"
echo "Ready: http://127.0.0.1:$OGH_SANDBOX_PORT — next time run ~/.local/bin/ogh-sandbox"
if command -v open >/dev/null; then open "http://127.0.0.1:$OGH_SANDBOX_PORT"; fi
exec msb exec --tty "$OGH_SANDBOX_NAME" -- bash /root/.local/bin/ogh-work

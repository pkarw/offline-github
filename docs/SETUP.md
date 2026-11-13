# Setup and troubleshooting

[← Getting started](../README.md) · [All Docs](README.md)

## One-command local setup

```bash
bash <(curl -fsSL https://raw.githubusercontent.com/pkarw/offline-github/main/scripts/setup.sh)
```

Use Bash or Zsh to paste this command. The downloaded script runs in Bash. It
first shows the project, CLI, cache and installer paths, then prompts for the
project folder before making changes. Press **Enter** to keep the default, or
type another name/path. After setup, it opens a **new interactive shell** in
your chosen folder (default `~/Projects/open-mercato-offline`), with
`OGH_REPO=open-mercato/open-mercato` and the CLI on PATH. `exit` returns to your
original shell; `~/.local/bin/ogh-work` returns to the project later.

The setup installs the CLI and UI, caches the complete public repository and
issue/PR feeds, creates a named development branch in a worktree backed by that
cache, clones the public skills collection, and selects `offline-github` in the
project's `.ai/agentic.config.json`. It installs project-scoped skills in the
Claude, Codex and pi discovery directories. Skills carry an explicit ogh
transport instruction; the real `gh` binary is not replaced or aliased.

It starts `ogh serve` in the background and verifies its health endpoint before
reporting success. Open http://127.0.0.1:4173. The server PID and output are in
`~/.local/share/ogh-setup/server.pid` and `server.log`. Stop only that PID when
finished (`kill "$(cat ~/.local/share/ogh-setup/server.pid)"` after checking it
still identifies your server). Restart with `ogh serve`.

## Prerequisites

- **macOS Apple Silicon:** Homebrew; missing Git, GitHub CLI, jq, Python and Node
  22 are installed using Brew. The app uses the checksummed v0.2.1 binary bundle,
  so Rust is unnecessary.
- **Other macOS/Linux:** Git, curl, GitHub CLI, jq, Python 3, Node.js 22+ and its
  npm, plus a native C build toolchain, pkg-config and OpenSSL development files.
  Rust is installed with the official rustup installer if absent. The CLI and UI
  are built from source. On Debian/Ubuntu:

  ```bash
  sudo apt-get install git curl gh jq python3 build-essential pkg-config libssl-dev
  ```

  Install Node.js 22+ using your preferred Node distribution before setup. Root
  Debian guests can have the apt prerequisites installed automatically. The
  installer never runs an implicit `sudo` command.
- Your normal GitHub login. If necessary, setup runs `gh auth login` interactively.
  It never copies credentials to the repository.
- Enough free space for full Git history, metadata, skills and build tools; first
  download can take several minutes. Keep the app-data cache and its worktrees.
- Install/sign into **pi**, **Codex** or **Claude Code** separately. Setup does
  not choose a paid agent, install an API key or read your agent credentials.

## What is ready afterward?

Start an agent in the shell opened by setup. Ask it to inspect an issue, write a
local fix, run tests or queue a comment with ogh. For PR review context, prepare
that PR while online:

```bash
ogh skills prepare --pr 123 --logs
```

Local skills obey the [offline tracker contract](trackers/offline-github.md).
Cached checks and identities are snapshots. A queued comment/label is not a
remote claim; live-only claim, merge and permission gates stop rather than
falling back to `gh`. Some complete delivery pipelines therefore cannot finish
offline. Skill source provenance is in the managed skills Git checkout and the
workspace `.ogh-workspace.json` manifest.

**This prepares source and agent skills, not a running ERP stack.** Install Open
Mercato's application dependencies, databases, Docker images, LFS assets and
browser test binaries separately using its own README before going offline.

## Existing folders, upgrades and customization

Setup preserves an existing destination unless it contains its own workspace
manifest. It never resets a branch, deletes a repository or force-pushes. A
managed workspace is reused without switching away from your work. Config
changes and refreshed managed skills keep timestamped backups. Existing
project-provided skill directories are preserved. Upstream project-specific
skills may override shared ones and still use the selected tracker descriptor.

Detailed installation, manual builds, upgrades, queue commands and recovery are
in the [app and CLI guide](GUIDE.md).

```bash
# 📂 Choose a folder directly (skips the folder prompt):
bash <(curl -fsSL https://raw.githubusercontent.com/pkarw/offline-github/main/scripts/setup.sh) --folder mercato-flight

# 👀 Preview every destination without installing anything:
bash <(curl -fsSL https://raw.githubusercontent.com/pkarw/offline-github/main/scripts/setup.sh) --folder "My Mercato" --plan
```

A bare `--folder NAME` goes under `~/Projects`; an absolute path is used as
given, and a path containing `/` is relative to your current directory. Quote
names with spaces. `-d` is shorthand for `--folder`. `OGH_WORKSPACE` sets the
default; `OGH_SETUP_PORT=4180` selects another browser port.

Interactive terminals prompt automatically unless `--folder` was supplied.
Use `--no-prompt` to accept the default in automation, or `--interactive` to
force a folder prompt (including with `--plan`). End-of-input cancels a prompt
before installation. The generated `ogh-work` helper remembers your folder.

`OGH_SETUP_ROOT` selects the installer/tool source directory. `OGH_DATA_DIR`
selects the application cache/outbox as usual; the helper retains its resolved path
between sessions. Existing helper files/symlinks are moved to unique backups.
`--no-shell` leaves the original shell in place; `--no-open` skips browser
opening. `--skip-sync` is only for reusing an already populated cache. The
installer source checkout refuses to update with local changes. An existing
skills clone is reused; refresh it deliberately and rerun to update copies.

If port 4173 belongs to a healthy ogh server using the same cache, setup reuses it.
A different cache, older unverifiable server or unrelated listener causes a clear error; choose another port. Check `server.log` on a
startup failure. An interrupted initial download can be rerun; an unrecognized
partial worktree is preserved and requires a new destination or manual inspection.

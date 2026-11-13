# Setup and troubleshooting

[← Getting started](../README.md) · [All Docs](README.md)

## One-command local setup

```bash
bash <(curl -fsSL https://raw.githubusercontent.com/pkarw/offline-github/main/scripts/setup.sh)
```

Use Bash or Zsh to paste this command. The downloaded script runs in Bash. It
first asks for a GitHub repository (**open-mercato/open-mercato** by default),
then shows destinations and prompts for the project folder before making changes. Press **Enter** to keep the default, or
type another name/path. After setup, it opens a **new interactive shell** in
your chosen folder (default `~/Projects/open-mercato-offline`), with
`OGH_REPO` set to your selected repository and the CLI on PATH. `exit` returns to your
original shell; `~/.local/bin/ogh-work` returns to the project later.

The setup installs the CLI and UI, caches the complete public repository and
issue/PR feeds, creates a named development branch in a worktree backed by that
cache, clones the public skills collection, and selects `offline-github` in the
project's `.ai/agentic.config.json`. It installs project-scoped skills in the
Claude, Codex and pi discovery directories. Skills carry an explicit ogh
transport instruction; the real `gh` binary is not replaced or aliased.

It starts `ogh serve` in the background and verifies its health endpoint before
reporting success. Your browser opens at the actual local URL (normally http://127.0.0.1:4173). The server PID and output are in
`~/.offline-github/setup/server.pid` and `server.log`. Stop only that PID when
finished (`kill "$(cat ~/.offline-github/setup/server.pid)"` after checking it
still identifies your server). Restart with `ogh serve`.

## Prerequisites

- **macOS Apple Silicon:** Homebrew; missing Git, GitHub CLI, jq, Python and Node
  22 are installed using Brew. The app uses the checksummed v0.2.3 binary bundle,
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

`--repo OWNER/REPO` (or `-R`) skips the repository prompt. A GitHub HTTPS URL
is accepted too. `OGH_SETUP_REPO` changes the default. For example:

```bash
bash <(curl -fsSL https://raw.githubusercontent.com/pkarw/offline-github/main/scripts/setup.sh) --repo acme/widget --folder widget-work
```

The default folder follows the repository name: `~/Projects/REPO-offline`.
Open Mercato skills are still installed with the ogh tracker for your selected
repository; framework-specific skills may only apply to Open Mercato projects.
An existing managed workspace for a different repository is preserved: choose
another folder to set up the new repository.

A bare `--folder NAME` goes under `~/Projects`; an absolute path is used as
given, and a path containing `/` is relative to your current directory. Quote
names with spaces. `-d` is shorthand for `--folder`. `OGH_WORKSPACE` sets the
default; `OGH_SETUP_PORT=4180` selects another browser port.

Interactive terminals prompt for each choice unless its `--repo` or `--folder`
option was supplied.
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
A different cache, older unverifiable server or unrelated listener is left alone;
a new server starts on a random free port. The actual URL is printed and saved
in `~/.offline-github/setup/server.url`. Standalone `ogh serve` also chooses a
free port and opens the browser; use `--no-browser` on headless machines. Check `server.log` on a
startup failure. An interrupted initial download can be rerun; an unrecognized
partial worktree is preserved and requires a new destination or manual inspection.


### 📂 Open the checkout from the app

The repository header shows its full local working path. **Open ▾** lists Claude
Code (`claude`), Codex CLI (`codex`), ogh CLI and Terminal. Commands are detected
on the server's PATH. Refresh detection after installing a tool; restart the
server if its PATH changed. If several Git worktrees exist, select the checkout
before opening. If only a bare cache exists, opening creates a named local branch
and working checkout under `<data-dir>/workspaces/owner/repo`.

Launches use Terminal on macOS, or an available desktop terminal on Linux.
The new shell retains the repository and cache identity. In a headless VM or
Microsandbox, desktop launching is unavailable: enter with `ogh-sandbox` and
start your agent there. The browser cannot launch host tools from a guest.

### Storage and local project removal

Repositories, settings, cached UI and outgoing changes live under `~/.offline-github` by default. Installer state is in `~/.offline-github/setup`; Microsandbox launch state is in `~/.offline-github/sandbox`. `OGH_DATA_DIR` overrides the app storage location. On upgrades, this directory may be a compatibility symlink to the existing platform cache, preserving live SQLite connections and Git worktree links without moving or deleting data. Your development checkout stays in the folder selected during setup.

Use **Pull from GitHub** to fetch incoming changes. **Push changes** previews only outgoing changes and requires confirmation; a zero outgoing count says nothing about incoming changes. Issues and pull requests can both be filtered by label.

**Remove offline repo** checks queued outgoing changes, uncommitted files and local commits against the last cached remote refs. It removes the project from the app list while retaining all files and recovery history. Export JSON or Markdown before removal if desired. Add the same repository again to restore its listing. This does not delete GitHub repositories or reclaim disk space.

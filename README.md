# 🛫 Offline GitHub

**Your repositories. Your issues. Your pull requests. Even at 35,000 feet.**

A local GitHub reading desk built with **Rust + React + shadcn/ui**. Sync before boarding, browse discussions in airplane mode, and check out a PR to test it locally. GitHub stays the source of truth.

![Offline GitHub workspace](docs/screenshots/workspace.png)

> 🚧 Early release. This is an independent offline client, not affiliated with GitHub.

## ✨ What you get

- 🗂️ Multiple projects in one sidebar, with searchable issues and pull requests.
- 💬 Cached issue bodies, comments and inline PR review comments, including closed work.
- 🔍 Local source browser and PR diffs computed from real Git objects.
- 🌳 Complete Git history, branches, tags and available PR head refs; no shallow clones.
- 🛬 Isolated PR worktrees for testing without touching another checkout.
- 🔄 Explicit delta sync with an overlapping timestamp window and atomic cache replacement.
- 🤖 A `gh`-shaped `ogh` CLI with JSON output for coding agents.
- 🔒 Upstream reads only. No issue edits, comments, merges or pushes. Cache remotes have pushing disabled.

## 🚀 Start here

Requires stable **Rust**, **Node.js 22+**, **Git**, and the [GitHub CLI](https://cli.github.com/). Authenticate `gh` before syncing; the app never stores your token. Public clones work over HTTPS; private Git clones need your normal Git credential helper (`gh auth setup-git` if desired).

```bash
git clone https://github.com/pkarw/offline-github.git
cd offline-github
npm --prefix web ci
npm --prefix web run build
cargo build --release --locked

# Optional: install the CLI in ~/.cargo/bin
cargo install --path . --locked

ogh repo add open-mercato/open-mercato
ogh repo add open-mercato/cezar
ogh sync
ogh serve --web-dir "$PWD/web/dist"
```

Open **http://127.0.0.1:4173**. Keep the local Rust server running while browsing offline. Every UI asset is served locally; no CDN, remote fonts or avatar requests are required.

The first sync downloads **all issues/PR metadata and conversations**, plus full Git history, and can take several minutes and significant disk space on large repositories. Later syncs request changed issues/comments and incrementally fetch Git objects. The PR endpoint has no `since` filter, so its update-sorted pages are scanned only as far as the last sync window.

<details>
<summary>📸 Pull requests, source browser and mobile</summary>

![PR diff](docs/screenshots/pull-request.png)
![Local source browser](docs/screenshots/source.png)
<img src="docs/screenshots/mobile.png" width="320" alt="Mobile workspace" />

</details>

## 🤖 Agent-friendly CLI

```bash
ogh repo list
ogh issue list -R open-mercato/cezar --state all --search sync --json
ogh issue view 123 -R open-mercato/cezar --json
ogh pr list -R open-mercato/open-mercato --limit 100 --json
ogh pr view 456 -R open-mercato/open-mercato --json
ogh pr diff 456 -R open-mercato/open-mercato
ogh pr checkout 456 -R open-mercato/open-mercato --dir ../pr-456

# Once online again:
ogh sync -R open-mercato/cezar
# Include review summary bodies/states (additional API request per PR):
ogh sync -R open-mercato/cezar --reviews
# Rebuild metadata to reconcile records deleted upstream:
ogh sync -R open-mercato/cezar --full
```

Replace example item numbers with numbers returned by `list`. All `list`, `view`, `diff` and `checkout` commands work from the completed local cache without GitHub access. `--json` is a boolean switch returning the complete cached representation, rather than gh's field-selection syntax. JSON list output is an array; detail output includes `item`, `comments`, `reviews`, `review_comments`, `files` and `diff`.

Checkout creates a **new detached worktree at the cached PR head SHA** and refuses an existing destination. To work on a fix, run `git switch -c my-fix` inside it. Offline GitHub never publishes local work; use your usual workflow when connected again.

## 🧠 How sync works

```text
GitHub REST API ── GET only ──▶ atomic JSON snapshot ──▶ UI / CLI
GitHub Git      ── fetch ─────▶ full bare repository ──▶ PR worktrees
```

The watermark records the start of a successful sync, with a two-minute overlap on the next request. Issues and comments are upserted by stable IDs. The snapshot and watermark are replaced only after all requested downloads succeed. A failed sync preserves the last complete snapshot; retry safely. Git objects fetched before a failure remain usable. One process at a time may mutate the cache; readers continue using the previous snapshot.

Data defaults to your OS's local application-data directory (`~/Library/Application Support/offline-github` on macOS, `~/.local/share/offline-github` on Linux). Override it with `OGH_DATA_DIR` or `--data-dir`. Nothing in the downloaded cache belongs in this public repository.

## 📋 Coverage and current limits

- Cached: issue/PR metadata, Markdown bodies, issue comments, inline review comments, all Git history reachable from advertised branches/tags/PR heads. Review summaries are opt-in with `--reviews` due to API cost.
- Deleted issues/comments are not reported by GitHub's delta feeds. Run `--full` periodically to remove stale cached records. The UI sync button runs the default delta sync.
- Git LFS payloads, submodule repositories, release assets, Actions logs/checks, attached images and linked websites are not downloaded. Markdown attachments need connectivity; source and text remain local.
- PR file names and diffs are derived locally. The UI diff is capped at 4 MB and file previews at 2 MB; `ogh pr diff` outputs the full patch. Unavailable historical base/head objects produce a visible diff error.
- Upstream branches refresh under `refs/remotes/origin/*`; browse `HEAD` or `origin/main` for the latest upstream code. Local branch refs are preserved so sync cannot overwrite your commits.
- No background network sync or service worker. The local server is required. Existing local working changes are never uploaded or overwritten.
- `ogh` intentionally supports the documented read-only subset of `gh`; it is not a drop-in replacement for every `gh` command.
- Local HTTP only, bound to `127.0.0.1`, with host/origin checks. Do not expose the server through a proxy to other machines: caches may contain private source and discussions.

## 🛠️ Development

```bash
bash scripts/check.sh
# UI development (Vite proxies /api to Rust):
ogh serve --web-dir "$PWD/web/dist"
cd web && npm run dev
```

The backend is Axum, Clap and an atomic on-disk JSON snapshot per project; Git is invoked directly without a shell. API calls go through authenticated `gh api --method GET`. API contract: [docs/API.md](docs/API.md).

**License:** [MIT](LICENSE) · Made for long flights and focused work ☁️

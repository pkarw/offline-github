# Offline GitHub — local API contract

Rust binary `ogh`; React/shadcn frontend under `web/`. Read endpoints return cached data. Explicit outgoing writes use the separate [outbox API](OUTBOX.md). Local server on 127.0.0.1:4173, same-origin. Production serves web/dist.

- GET /api/repos → array of `{full_name,description,default_branch,last_sync,issue_count,pr_count,path}`. last_sync nullable ISO date, counts integers. path is local git mirror path.
- POST /api/repos body `{full_name:"owner/repo"}` → register only, returns repo entry. Explicit sync downloads data.
- POST /api/repos/:owner/:repo/sync → synchronous explicit sync, `{message,changed,last_sync}`; UI show pending and catch errors.
- GET /api/repos/:owner/:repo/items?kind=issue|pr&state=open|closed|all&q=search → GitHub-shaped issue array (number,title,body,state,user:{login},labels:[{name,color}],comments,created_at,updated_at,pull_request optional). merged_at optional on PRs.
- GET /api/repos/:owner/:repo/items/:number → `{item,comments,reviews,review_comments,files,diff}`. item is GitHub issue or detailed PR (head.ref,base.ref,merged_at,draft), comments/reviews/review_comments GitHub arrays, files GitHub changed-file array, diff string.
- GET /api/repos/:owner/:repo/tree?ref=HEAD&path= → array `{name,path,kind:"tree"|"blob"}`.
- GET /api/repos/:owner/:repo/blob?ref=HEAD&path=README.md → `{content}` plain string.
- GET /api/health → `{status:"ok",read_only:true}`.
- Errors: non-2xx `{error:string}`.

CLI reads/checkout: ogh repo add owner/repo; ogh sync [-R owner/repo]; ogh issue list/view NUMBER -R owner/repo [--json]; ogh pr list/view NUMBER/diff NUMBER/checkout NUMBER -R owner/repo [--json] (checkout uses --dir PATH to create isolated local worktree). UI displays/copies checkout CLI command; it never silently changes local code. All assets must work offline; no CDN/fonts/remote avatars. Markdown sanitized (no raw HTML). Empty cache is real empty state, never fake data. User wants multi-project left pane, polished GitHub-like UI and full detail/discussion/diff + code browser. README/screenshots owned by parent.


Global CLI supports `--repo`/`-R`, `OGH_REPO`, gh-style `--json FIELD,...` and `--jq`, skill preparation/status, installed app assets and a durable outgoing queue. See README and OUTBOX.md. Read endpoints and `/api/repos/:o/:r/sync` remain incoming-only. Health reports `mode: offline_first` and `outgoing_queue: true`.

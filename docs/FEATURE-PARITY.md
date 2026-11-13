# gh and ogh feature parity

[← All Docs](README.md) · [Getting started](../README.md#getting-started--one-command)

`ogh` v0.2.1 implements a focused subset of GitHub CLI commands for offline work.
It is not a drop-in replacement for every `gh` command or flag. GitHub remains
the source of truth. **Cached** means data from the last sync/preparation;
**queued** means a durable local intention until reviewed Sync confirms delivery.

| Feature / command family | gh | ogh | Practical difference |
| --- | --- | --- | --- |
| Authentication | `auth login/status` | Uses real `gh` for online sync | No independent token management or offline authentication proof. |
| Repositories | `repo list/view/clone/create/...` | `repo add/list/view`, Git cache | List covers added repositories; no repository creation/deletion/admin. |
| Issues | `issue list/view` | Cached list/view | Supported fields and search subset; freshness is explicit. |
| Pull requests | `pr list/view/diff` | Cached list/view; local Git diff | Full history/PR refs after sync; extra review/CI data needs preparation. |
| Search | `search issues/prs`, GitHub search | `search issues/prs` and list filters | Local repository scope; supported qualifiers only; no OR or arbitrary GitHub search syntax. |
| JSON and filtering | `--json`, `--jq` | Selected gh-style fields, `--json`, `--jq` | Not every gh field/flag exists; jq must be installed. |
| REST API | Arbitrary authenticated REST requests | Allowlisted cached GET resources | Includes items, comments, reviews, commits, files, labels, contents and prepared CI data; unsupported routes fail. |
| GraphQL | `api graphql` | Unsupported | PR commit count uses prepared REST `.commits`; no arbitrary query evaluation. |
| Pagination | `api --paginate` | Supported cached collections | Returns captured data, never fetches missing pages on an offline read. |
| PR checkout | `pr checkout` | `pr checkout --dir PATH` | Creates an isolated detached worktree; does not switch the current checkout. |
| Local branches / commits | Native Git | Native Git plus `branch create`, `commit` | Local work works offline. Branch publication is a separate queued action. |
| Issue/PR creation | `issue create`, `pr create` | Queued equivalents | Local IDs/receipts first; GitHub numbers/URLs only after confirmation. |
| Comments | `issue comment`, `pr comment` | Queued equivalents | Pending comments appear in the UI with sync status. |
| Issue/PR title, body, state edits | Direct online edits | Queued `issue edit`, `pr edit` proposals | Safe default posts an edit proposal for manual application; it does not overwrite concurrently edited fields. |
| Labels | List/create/edit/delete/clone; apply via edit | Cached `label list`, queued `label create` and add/remove deltas | No label taxonomy edit/delete/clone support. |
| Reviews | `pr review`, review data | Cached reviews; queued `pr review` | Queued review is not a submitted review or live approval receipt. |
| Checks | `pr checks` | Cached `pr checks` | Failure/pending return non-success; stale/missing head coverage fails. No live watch. |
| Actions runs / logs | `run list/view/watch/rerun/...` | Prepared `run list/view --log-failed` | Only prepared PR-head coverage; no run/rerun/cancel/watch or artifact downloads. |
| Assignments, claims, locks | Online issue/PR edits and APIs | Unsupported live operations | Cached identity/labels never acquire ownership. Skills requiring live claims stop. |
| Merge / ready / branch protection | Online commands/APIs | Unsupported | No offline merge authorization or protection administration. |
| Releases, gists, projects, discussions, secrets, organizations, Codespaces, extensions | Dedicated online command families | Unsupported | Use real gh online when explicitly appropriate. |
| Incoming updates | Live reads | `sync` / UI refresh | Delta metadata updates plus Git fetch; `--full` for full refresh. |
| Outgoing audit and recovery | No equivalent durable offline ledger | `outbox list/report/resolve/sync` | JSON/Markdown payloads, receipts, conflicts and history; preview before transmission. |
| Local browser app | Opens GitHub pages | `serve` | Local React app works with cached data, including pending-item markers. |
| Agent skills | Online workflows | Project-scoped ogh tracker/skills setup | Local analysis/code/tests work; live publication/claim/merge gates remain live. |

## What to prepare before a flight

```bash
ogh sync -R open-mercato/open-mercato
ogh skills prepare -R open-mercato/open-mercato --pr 123 --logs
ogh skills status -R open-mercato/open-mercato --pr 123
```

Replace `123` with the PR you plan to work on. Install project dependencies,
services and agent tools separately while online. Read-only commands do not
silently fall back to GitHub. Sending queued changes requires explicit reviewed
outbox Sync; ordinary incoming sync does not publish your work.

Command/field details: [offline tracker adapter](trackers/offline-github.md),
[skill operation audit](skills-compatibility.md), [outgoing safety](OUTBOX.md).
The gh column follows the [official CLI manual](https://cli.github.com/manual/gh)
and [API manual](https://cli.github.com/manual/gh_api), checked 2026-11-13.
The ogh column follows this repository's command definitions and regression tests.

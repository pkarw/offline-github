# Tracker provider: Offline GitHub (ogh)

This provider supports **offline reads, durable queued writes, and reviewed online
Push**. It does not bypass any skill's claim/QA/merge gate.
Use it for local analysis, specifications, reviews saved locally, code changes,
local tests, and explicitly queued outgoing intentions. GitHub remains master. Do not replace `gh` globally or alias it:
`ogh sync` and `ogh skills prepare` use the real authenticated `gh` while online.

## Prerequisites and provenance

Before boarding: `ogh sync -R owner/repo`, then
`ogh skills prepare -R owner/repo --pr NUMBER --logs` for each PR to review.
The latter captures reviews, merge metadata, labels, check runs, workflow runs,
required-check protection when readable, jobs and failed job logs. It does not
install Node dependencies, Docker images, browsers, LFS files or submodules.
Provision the repository's local test environment separately while online.

**auth-check** becomes `ogh skills status -R owner/repo --pr NUMBER`: this checks
the offline cache, NOT current authentication. Read all cache timestamps and
`head_sha` coverage. Cached green checks, review decisions and lock absence are
never fresh evidence authorizing a merge, claim or publication. Missing data is
an error, never an empty successful result. Reconcile with real GitHub online.

## Read operations

Pass `-R owner/repo` explicitly or set `OGH_REPO`; GitHub origin inference also
works. `--json FIELD,...` uses gh-style field names; `--jq` requires local jq.
Bare `--json` retains legacy ogh raw output, so use explicit fields in skills.

| Tracker operation | Command |
| --- | --- |
| current-user | `ogh api user -R owner/repo --jq .login` (cached identity only) |
| repo-info / default-branch | `ogh repo view --json nameWithOwner,defaultBranchRef` |
| get-issue | `ogh issue view NUMBER --json number,title,body,state,author,url,labels,assignees,comments` |
| search-issues | `ogh issue list --state all --search 'text in:title,body' --json number,title,url` |
| get-pr / list-prs | `ogh pr view NUMBER --json number,title,body,author,baseRefName,headRefOid,reviewDecision,mergeable,mergeStateStatus` / `ogh pr list --state open --json number,title,labels,headRefName` |
| search-prs | `ogh pr list --state all --search 'is:merged merged:>=2026-01-01' --json number,title,mergedAt,author` |
| get-issue-comment / get-pr-comment | `ogh api repos/OWNER/REPO/issues/comments/ID` |
| list-issue-comments | `ogh api repos/OWNER/REPO/issues/NUMBER/comments --paginate` |
| get-review-comment / list-review-comments | `ogh api repos/OWNER/REPO/pulls/comments/ID` / `ogh api repos/OWNER/REPO/pulls/NUMBER/comments --paginate` |
| get-pr-diff / get-pr-files | `ogh pr diff NUMBER` / `ogh pr diff NUMBER --name-only` / `ogh api repos/OWNER/REPO/pulls/NUMBER/files --paginate` |
| get-pr-commits | `ogh api repos/OWNER/REPO/pulls/NUMBER/commits --paginate` |
| checkout-pr | `ogh pr checkout NUMBER --dir ../offline-pr-NUMBER --recurse-submodules=no` (new isolated detached worktree; never changes the current checkout) |
| get-pr-checks | `ogh pr checks NUMBER --json name,state,link` (snapshot only) |
| get-required-checks | `ogh api repos/OWNER/REPO/branches/BRANCH/protection/required_status_checks --jq '.contexts[]'` (errors when unreadable/not prepared) |
| list-runs / get-run | `ogh run list --branch BRANCH --json databaseId,workflowName,status,conclusion,headSha,url` / `ogh run view ID --json status,conclusion,jobs` |
| get-run-failed-logs | `ogh run view ID --log-failed` (requires `prepare --logs`) |
| list-labels | `ogh label list --limit 200 --json name --jq '.[].name'` |
| repository contents | `ogh api 'repos/OWNER/REPO/contents/PATH?ref=HEAD' --jq .content` (base64 text) |

API reads are served only from local files/Git, never forwarded online. GraphQL
execution, arbitrary endpoints and unsupported search qualifiers fail explicitly.
Search supports text/quoted phrases, `in:title,body,comments`, `is:`, `state:`,
`author:`, `assignee:`, `label:`, `base:`, `head:`, `repo:`, `number:`, `draft:`,
`created:`, `updated:`, `closed:`, `merged:` and leading `-` negation. OR and other
GitHub search operators are unsupported. Workflow coverage includes only prepared
PR heads, not every Actions run in the repository.

## Queued write operations (local intent, not remote tracker state)

Comments and labels **are supported offline**. Translate skill examples that use
`gh` or mutating REST requests into these dedicated commands; `ogh api` is read-only.
Pass `-R owner/repo` on every command (or set `OGH_REPO`).

| Outgoing operation | Queue locally |
| --- | --- |
| Comment on an issue / PR | `ogh issue comment NUMBER --body-file FILE` / `ogh pr comment NUMBER --body-file FILE` |
| Add / remove issue or PR labels | `ogh issue edit NUMBER --add-label NAME --remove-label OTHER` / `ogh pr edit NUMBER --add-label NAME` |
| Create repository label | `ogh label create NAME --color aabbcc --description 'Description'` |
| Create issue | `ogh issue create --title TITLE --body-file FILE` |
| Create PR | `ogh pr create --title TITLE --body-file FILE --head BRANCH --base main` |
| Submit PR review | `ogh pr review NUMBER --comment --body-file FILE` (also `--approve` / `--request-changes`; approval still requires the skill's review gates) |
| Publish local branch | `ogh branch publish BRANCH` (no force push) |
| Propose title/body/state edit | `ogh issue edit NUMBER --title TITLE` / `ogh pr edit NUMBER --title TITLE` (safe proposal; never silently overwrites upstream fields) |

These commands return local intentions, not live GitHub receipts. Field edits
preserve proposals under the default no-overwrite policy. To publish when online
and authorized:

1. Run `ogh outbox sync --dry-run -R owner/repo` and inspect every outgoing payload.
2. Use its `plan_token`: `ogh outbox sync --yes --plan-token TOKEN -R owner/repo`.
3. Inspect the returned statuses and GitHub receipts. Unresolved conflicts and
   uncertain results remain queued for explicit recovery, not blind resending.

The UI calls this **Push**; **Pull** (`ogh sync`) downloads incoming GitHub data
and does not publish the queue. `ogh outbox report --format json` or `--format md`
exports payloads, conflicts, receipts and audit history for recovery. Read
[the outgoing contract](../OUTBOX.md). Do not claim these supported writes are
unavailable merely because the workspace uses this provider.

Queued assignment/labels/comments never satisfy a mandatory LIVE claim step.
Do not report a PR published, review submitted or issue created until its event
has an actual GitHub confirmation. Do not overwrite the original GitHub master
snapshot with local intentions. Root-cause/local review may continue; an auto
pipeline requiring a fresh claim must stop or resume online.

For commit attribution, after preparation use
`ogh api repos/OWNER/REPO/pulls/NUMBER --jq .commits` for the independently captured
REST commit total, and paginated `/pulls/NUMBER/commits` for identities. Preparation
asserts count equality. Replace the skill's narrow GraphQL total-count lookup
with this explicit offline alternative; arbitrary GraphQL is not emulated.

## Unsupported live operations

Do not run acquire/release-lock, claim-reset/winner/withdraw, assign/unassign,
ready/merge PRs,
approve/rerun/watch workflows, image/session publication or branch
protection changes through this provider. Branch publication is supported through
the reviewed outgoing queue described above. There is no local lock emulation. The outgoing queue does not authorize these
live-only operations or make queued intentions remotely visible. If a skill requires one of these operations before
implementation, stop that workflow and do explicitly requested local preparation
instead. Save findings/patches locally and resume the actual skill online.

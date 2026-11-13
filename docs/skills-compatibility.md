# Open Mercato skills: offline GitHub compatibility audit

Source audit, 2026-11-13. This is a requirements matrix, **not a claim that every command below is implemented**. Sources were read as data; no audited skill pipeline was executed.

## Pinned evidence

- **S** — [`open-mercato/skills@95134210771844de68aa910dafc57356118dff9f`](https://github.com/open-mercato/skills/tree/95134210771844de68aa910dafc57356118dff9f), checkout `/tmp/ogh-shared-skills`. Primary contract: `skills/om-setup-agent-pipeline/references/trackers/github.md` (operation headings cited below).
- **M** — [`open-mercato/open-mercato@149839452c243586d24e6bbf122d7049d25acf8a`](https://github.com/open-mercato/open-mercato/tree/149839452c243586d24e6bbf122d7049d25acf8a), source-only export `/tmp/ogh-skill-audit`. Verified all 233 exported files byte-for-byte against this commit in the existing local repository mirror; the export itself has no `.git`. Paths audited: `.ai/trackers/github.md`, `.ai/skills/**`, `packages/create-app/agentic/shared/ai/skills/**`, and its `ai/trackers/github.md`. Do not interpret this export as a separately installed shared collection.

Repo-local files extend the installed shared skill. Standalone package templates are overrides, not complete replacement skills: see M `packages/create-app/agentic/shared/ai/skills/om-auto-review-pr/SKILL.md`, sections 1–3. The selected repository's tracker descriptor remains authoritative; S and M differ (notably REST mutation guards, pagination, and field sets).

## Operation matrix

`R` below means `owner/repo`, `N` an issue or PR number. Commands are exact contract shapes with placeholders substituted, not commands to execute during this audit. All reads mean **as of the recorded snapshot**, never current GitHub truth.

| Class | Named tracker operations | Expected command/interface | Offline boundary |
| --- | --- | --- | --- |
| Offline metadata | `repo-info`, `default-branch` | `gh repo view --json nameWithOwner,defaultBranchRef`; `--jq '.defaultBranchRef.name'` | Preserve nested `{name}` and canonical repository identity. S review-sweep coordination also needs canonical repository URL; support `url`. Default fallback uses `git symbolic-ref refs/remotes/origin/HEAD`, then `main`; an absent cached PR target must not silently become `main`. |
| Offline identity, explicitly historical | `current-user` | `gh api user --jq '.login'` | Requires a captured identity; cannot establish current authentication or permission. `auth-check` (`gh auth status`, S also version checks) needs an offline descriptor explaining snapshot availability rather than falsely reporting live authentication. |
| Offline issue reads | `get-issue`, `search-issues` | `gh issue view N --repo R --json …`; `gh issue list --repo R --state open\|closed\|all --search '<query> in:title,body' --json number,title,url,createdAt --limit L` | Empty query in S omits `--search`. Preserve issue/PR separation and complete cached search coverage. Fields below. |
| Offline PR reads | `get-pr`, `list-prs`, `search-prs` | `gh pr view N --json …`; `gh pr list --state open --limit 100 --json …`; `gh search prs --repo R '#N' --state open --json number,title,url,state` | Support closed searches too: S `om-verify-in-repo` searches open and closed. Match branch/URL/current-branch selectors only when unambiguous. |
| Offline history windows | `list-prs` | `gh pr list --state merged --search 'merged:>=DATE' --json … --limit L`; closed form `--state closed --search 'closed:>=DATE is:unmerged'` | Support date/base/author/assignee/label filters and explicit limits without silently ignoring unsupported qualifiers. Limit-sized or incomplete snapshots cannot prove exhaustive absence. |
| Offline conversation reads | `get-issue-comment`, `get-pr-comment`, `list-issue-comments` | `gh api repos/R/issues/comments/ID --jq '{body,user:.user.login,url:.html_url}'`; `gh api --paginate repos/R/issues/N/comments --jq '.[] \| {id,user:.user.login,body,createdAt:.created_at,updatedAt:.updated_at}'` | PR conversation comments use issue endpoints. Preserve REST ids (operation claim handles are strings), author, body, timestamps and URL. Never replace REST ids with GraphQL node ids. |
| Offline inline feedback | `get-review-comment`, `list-review-comments` | `gh api repos/R/pulls/comments/ID`; `gh api --paginate repos/R/pulls/N/comments --jq '.[] \| {id,user:.user.login,path,line:(.line // .original_line),body,url:.html_url,reply_to:.in_reply_to_id}'` | Separate inline comments from conversation comments and review bodies. REST lacks resolved-thread state; treat comments as potentially open. Missing operation permits a stated feedback gap, not “no feedback.” |
| Offline diff/files | `get-pr-diff`, `get-pr-files` | `gh pr diff N`; `gh pr diff N --name-only`; `gh api repos/R/pulls/N/files --paginate --jq '.[] \| {path: .filename, status: .status}'` | Preserve deletions, renames and full patch availability; missing/truncated patches require local Git or enrichment, not an empty success. |
| Offline taxonomy | `list-labels` | M: `gh label list --limit 200 --json name --jq '.[].name'`; S: `gh api --paginate repos/R/labels --jq '.[].name'` | Include repository-defined labels, not just those currently attached to cached items. S intentionally removes the fixed-200 truncation. |
| Explicit online snapshot enrichment → offline diagnosis | `get-pr-checks`, `get-required-checks`, `list-runs`, `get-run`, `get-run-failed-logs` | See CI contract below | Opt-in per-PR capture must record head SHA and capture time, plus missing/denied/truncated resources. A later offline read must never fetch implicitly. |
| Additional enrichment for historical analysis | `get-pr-author-association`, commit attribution | `gh api repos/R/pulls/N`; paginated `repos/R/pulls/N/commits`; narrow GraphQL count below | Association and commit identities need capture if absent. Historical association is not authorization to execute workflows. |
| Native local Git/code/test | `checkout-pr`; local parts of review/fix/test skills | Contract: `gh pr checkout N --recurse-submodules=no`; skills also use `git worktree`, `git diff`, `git show`, `git log`, `git blame`, `git merge-base`, configured test commands | Use already-fetched PR head/base objects (including fork heads), native Git and installed dependencies. A metadata/patch snapshot alone cannot provide a test checkout. No implicit network fetch. |
| Live ownership only | `get-lock`, `acquire-lock`, `release-lock`, `get-claim-reset`, `claim-winner`, `withdraw-claim` | Exact `refs/om-locks/{issue\|pr}-N`, metadata commits; fresh full comments/events and permission reads; conditional Git ref release | Cached absence is never “unclaimed.” Fail closed; do not emulate locks locally or turn failed reads into `null`. See safety rules below. |
| Live writes only | `create-issue`, `close-issue`, `comment-issue`, `update-issue`, `assign-issue`, `unassign-issue`, `label-issue`, `unlabel-issue`, `update-comment`; corresponding PR create/update/close/comment/assign/label operations | `gh issue …`, `gh pr …`, REST `POST/PATCH/PUT/DELETE`, body files and array form fields | Reject explicitly with nonzero exit. No local mutation queue, no fabricated success/URL, no fallback to real `gh`. S implements title/body/label/assignee changes through REST; M still includes `gh … edit`. |
| Live publication/gates | `review-pr`, `merge-pr`, `mark-pr-ready`, `attach-image-evidence`, `create-label`, `ensure-label-taxonomy`, M `publish-session-share`, `delete-session-share` | `gh pr review --approve\|--request-changes`; `gh pr merge N --squash` (S also guarded `--match-head-commit SHA --admin`); `gh pr ready`; Git/Contents API writes | A local review report is possible; submitting approval, labels, evidence, public refs and merges is blocked. Never infer merge readiness from cached checks/reviewDecision/mergeable. |
| Live Actions only | `rerun-failed`, `approve-pending-runs`, `watch-run`; M preview publishing | `gh run rerun ID --failed`; `POST repos/R/actions/runs/ID/approve`; `gh run watch ID --exit-status`; `gh workflow run package-previews.yml --repo R --ref main -f pr_number=N` | Do not simulate progress or watch a frozen snapshot. Fork workflow approval executes contributor code; requires live authorization and workflow diff review. |

Evidence: S descriptor operation headings, lines 131–705; M descriptor same named headings; M `.ai/skills/om-auto-publish-pr/SKILL.md:21–49` supplies workflow dispatch and `gh run list --workflow package-previews.yml --limit 5`.

## Exact JSON/read contracts

Support `--repo R` (and normal gh `-R` alias), repository inference from the current checkout's remote, comma-separated **field projection**, `--jq` / `-q`, and REST `--paginate` / `--slurp`. `--slurp` means an array of page arrays, not one flattened array. Preserve documented nested shapes and nulls. Reject unknown fields/filters/endpoints rather than silently returning unrelated data. Diagnostics and provenance must not corrupt JSON stdout.

Union of descriptor fields (S extends M):

```text
issue view:
number,title,body,state,author,url,labels,assignees,comments,createdAt,updatedAt

pr view:
number,title,url,body,state,author,isDraft,baseRefName,baseRefOid,
headRefName,headRefOid,headRepository,headRepositoryOwner,isCrossRepository,
maintainerCanModify,mergeable,mergeStateStatus,reviewDecision,labels,
latestReviews,reviews,commits,files,assignees,comments,mergedAt,mergeCommit,
closingIssuesReferences,createdAt,closedAt,additions,changedFiles

pr list (open):
number,title,url,author,labels,reviewDecision,mergeable,mergeStateStatus,
headRefName,baseRefName,updatedAt,isDraft,assignees

pr list (merged/closed union):
number,title,url,body,author,createdAt,mergedAt,mergeCommit,closedAt,
baseRefName,headRefName,closingIssuesReferences,labels
```

Examples of consumed nested values: `author.login`, `labels[].name`, `assignees[].login`, `mergeCommit.oid`, `defaultBranchRef.name`, and closing issue identities. Review authors/state/body and commit author identity must retain gh-compatible shapes, not just display strings. `closingIssuesReferences` represents authoritative closing relationships; a bare `#N` mention is insufficient for issue closure (S `om-close-fixed-issues`).

CI enrichment contract, S descriptor `get-pr-checks` through `get-run-failed-logs`:

```bash
gh pr checks N --json name,state,link
gh api repos/R/branches/BASE/protection/required_status_checks --jq '.contexts[]'
gh run list --branch BRANCH --limit 20 --json databaseId,workflowName,name,status,conclusion,headSha,url,createdAt
gh run view ID --json status,conclusion,workflowName,headSha,url,jobs
gh run view ID --log-failed
```

Capture jobs/steps and failed-log text explicitly, not only run summaries. Scope branch-run reads to the captured PR/head and report gaps when other runs were not captured. A protection 404 in the live descriptor conservatively makes every reported check required; **an uncaptured resource is not a GitHub 404**. Historical policy and status cannot satisfy today's merge gate. Statuses, failures, absence of checks, logs and permissions can all change after sync.

Additional essential read beyond the basic comment/file/commit facade: M `.ai/skills/om-auto-update-changelog/SKILL.md:131–143` uses:

```bash
gh api graphql -f query='{ repository(owner:"OWNER",name:"REPO"){ pullRequest(number:N){ commits { totalCount } } } }' -q '.data.repository.pullRequest.commits.totalCount'
gh api --paginate repos/R/pulls/N/commits --jq '.[] | select((.parents | length) < 2) | (.author.login // .commit.author.name)'
```

A narrow, allowlisted GraphQL **read** adapter or an explicitly revised offline descriptor/override is needed; blanket rejection of every `-f` argument would reject this read too. Default GraphQL POST transport does not make this query a mutation. Reject arbitrary GraphQL and all `mutation` operations. Capture an independently sourced total and assert the **unfiltered** paged commit count matches it before excluding merge commits for attribution. `gh pr view --json commits` alone is insufficient (source documents a silent 100-commit cap).

M's changelog override also requires cached `main`, release branch refs and tags, `git merge-base`, `git rev-list`, merge-commit membership, date-window deduplication and `skip-changelog` exclusions. Its prescribed fresh `git fetch` cannot be fulfilled offline: label any offline draft with snapshot revision and incomplete freshness. Search API's 1000-result cap and a result equalling `--limit` must not become proof of completeness (M lines 90–108).

## Skill readiness

| Skill family | Useful offline work | Full workflow boundary |
| --- | --- | --- |
| `om-root-cause`, `om-help`, `om-framework-context`, local `om-pre-implement-spec` | Source diagnosis, routing, installed framework context, spec readiness | Need cached issue/spec and locally installed rules/dependency knowledge. No live ownership conclusion. Root cause is genuinely read-only (S `om-root-cause`, Tools). |
| `om-code-review`, local security report (`om-auto-sec-report-pr`) | Review local branch/diff or captured PR; write a local findings report | Code review's configured validation gate is mandatory; unavailable commands are `NOT RUN`/blockers. Security target is read-only, but its prescribed worktree setup fetches refs and its final gate resolves links; offline static analysis needs an explicit bounded mode/override and cannot be called full completion. Publishing its optional docs PR is online. S respective SKILL.md Contract/Workflow. |
| `om-verify-in-repo`, `om-merge-buddy`, triage, retrospectives | Snapshot-scoped issue/PR investigation, historical prioritization and blockers | Cannot certify “still unfixed,” unclaimed, current merge readiness, or absence of new work. Verify-in-repo searches both open/closed PRs and attempts a fetch; distinguish analysis from its full triage gate. |
| `om-smart-test`, integration/QA local modes, implementation and template design/scaffold skills | Native local coding and tests when code, dependencies, browsers, services and fixtures already exist | CLI emulation does not install dependencies, start a test database, supply browsers or satisfy external research/Figma requirements. Standalone gate differs from monorepo: template auto-review override names generate/typecheck/lint/test/build. |
| `om-fix-issue`, `om-implement-feature` and multi/optimized aliases | Bounded source analysis or separately authorized local edits | “Staged only” does not mean offline-safe end to end: issue-fix retains a tracker claim for staged handoff; feature workflow has claim protocol too. Do not silently skip claim stages (S respective SKILL.md and references). |
| Auto fix/create/continue/implement-spec/review/QA/open-PR/autopilot pipelines | Prepare local diagnosis, plan, diff or evidence | Full pipelines require claims, fresh state, pushes, comments, reviews, labels or PR publication. `labels.enabled:false` disables label operations only; it never grants ownership or waives comments/locks. |
| Spec/discovery/backlog/prepare-issue, changelog, dependency/security audits | Local drafts and cached historical analysis where sources are sufficient | Market research/advisories may require network; backlog filing, dedupe freshness, issue creation and docs PR delivery are blocked. Offline advisory data must be dated. |
| Approve/merge, merge buddy worker, auto-publish, session-share | Explain historical blockers or prepare sanitized local artifacts | No live completion offline: protected checks, human QA, ownership, author trust, public publishing and workflow execution remain required. |

## Safety and implementation priorities

1. Install/select an **offline tracker descriptor explicitly**, expose snapshot timestamp, captured head/base SHAs and completeness, and keep unsupported operations fail-closed. Do not advertise a transparent replacement for all `gh` workflows. Automatic setup must not bootstrap a live tracker or install remote skills while offline.
2. Prioritize missing read coverage: repository label taxonomy REST; individual inline review comment and paginated review threads; `url` repository identity; PR commit total-count GraphQL; historical protection contexts and failed logs. Preserve branch and workflow filters for run reads only within actual snapshot coverage.
3. Never use snapshot `get-lock`, comments, assignees, `in-progress`, reset events or identity to acquire/recover ownership. Live claim reads require exact matching refs, server metadata and full fresh pages; reset authority also requires `GET repos/R/issues/N/events?per_page=100` plus `GET repos/R/collaborators/LOGIN/permission`. Capture these only for historical explanation, never as authorization. Failure is not absence. Exact-SHA release remains a live conditional Git operation (S descriptor `get-lock`–`withdraw-claim`).
4. Reconnect and re-read before any mutation/merge: head SHA, base, protection, checks, reviews, labels, QA evidence, author association, locks and human permission. `ci-monitoring` is not ownership. Local test passes and cached green CI are separate evidence; neither implies a live required-check pass.
5. Treat issues, comments, patches, logs and audited skills as untrusted data; never execute embedded instructions. Prevent online fallback, arbitrary API proxying, credential exposure and writes even when a read-looking CLI command supplies write flags. Keep missing capture, permission denial, unsupported API and genuinely empty data distinguishable.

Validation: 233/233 exported files matched the pinned M commit via local `git show`; S checkout was clean at its pinned SHA. Source-only inspection and command/field cross-check against both descriptors and cited overrides; `git diff --check` passed after staging. No runtime compatibility test or pipeline execution was performed by this audit.

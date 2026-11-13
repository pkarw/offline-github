# Offline writes / UI contract

GitHub stays master; local modifications are durable queued intentions. SQLite
outbox uses WAL, FULL synchronous commits, immutable audit rows and no automatic
deletion of applied operations. No implicit upstream mutation on a read or pull.

## API (same localhost origin/JSON guard)

- GET `/api/outbox?repo=owner/repo&target=123` → `{events:[Event],counts:{queued,sending,applied,conflict,failed,uncertain,cancelled}}`. Filters optional; all statuses retained. Also supports `?format=md` → Markdown text report, default JSON. A downloadable report is the complete event/audit payload, including local bodies and conflicts.
- POST `/api/outbox` JSON `{repo,kind,target?,payload,depends_on?:[]}` → Event. Queues locally only. target is string item number (or `local:<event-id>` for a pending new issue/PR). `kind` / payload:
  - `comment`: `{body}`
  - `edit`: `{item_kind:"issue"|"pr",fields:{title?,body?,state?}}`
  - `labels`: `{add:[names],remove:[names]}`
  - `label_create`: `{name,color,description}`
  - `issue_create`: `{title,body,labels?:[names]}`
  - `pr_create`: `{title,body,head,base,draft?:bool}`
  - `review`: `{body,event:"COMMENT"|"APPROVE"|"REQUEST_CHANGES",commit_id}`
  - `branch_push`: `{branch,sha}` (usually queued from CLI; commit SHA must already exist locally)
- POST `/api/outbox/sync` `{repo?,dry_run?:bool}` → `{events,counts,incoming_errors}`. dry_run true returns plan without network or upstream writes. Actual sync reconciles interrupted sends before retry, checks remote conflicts, sends queued work, then pulls each selected repo; cache-refresh failures are reported without undoing confirmed outgoing results.
- POST `/api/outbox/:id/resolve` `{action:"retry"|"use_local"|"keep_remote"}` → Event. retry requeues failed/uncertain sends for reconciliation; use_local explicitly rebases a conflicting edit against the last observed remote fields, preserving old base/payload in audit; keep_remote cancels the local intent but preserves it and all history in report.

Event: `{id,seq,repo,kind,target:string|null,payload,base,depends_on,status,created_at,updated_at,attempts,remote:null|GitHubResponse,error:null|string,observed:null|object,history:[{at,action,data}]}`.

## UI requirements

Add Outgoing workspace screen with count badge, all-event table/cards, status
filters, error/conflict details, local/observed side-by-side for conflicts, actions
above and JSON/Markdown downloads. Pending status must be visible beside queued
comments/labels/body edits; append local comment cards to matching issue/PR detail
and distinguish queued/sending/conflict/failed/uncertain/applied. Applied comment
cards may remain until their returned remote id appears in cached comments; never
render duplicates. Label changes need clear pending marker/chips.

Provide local comment composer; edit title/body action; add/remove issue labels;
new issue and repository label dialogs. PR creation may use a simple dialog with
head/base (users create commits/branches via Git/CLI). Selected repo's Sync button
opens outgoing preview and explicit "Sync N changes" action (or pulls only when no
outgoing changes); show network errors and unresolved conflicts, never claim full
success with pending operations. Do not run Sync automatically in tests.

Parent owns backend/CLI/docs. Frontend task owns web/ only. Existing cached API
shapes remain; frontend obtains outbox separately and refreshes after writes.

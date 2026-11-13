import React, { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import * as Dialog from "@radix-ui/react-dialog";
import * as Tabs from "@radix-ui/react-tabs";
import {
  Plus,
  RefreshCw,
  ArrowDownToLine,
  ArrowUpFromLine,
  Search,
  GitPullRequest,
  CircleDot,
  Code2,
  ChevronRight,
  Folder,
  FileText,
  ArrowLeft,
  Copy,
  Check,
  WifiOff,
  MessageSquare,
  GitBranch,
  X,
  ExternalLink,
  Menu,
  AlertCircle,
  Inbox,
} from "lucide-react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Button } from "./ui";
import { readRoute, routePath, numberQuery, type Route } from "./navigation";
import {
  api,
  repoUrl,
  type Repo,
  type Item,
  type Detail,
  type Entry,
  type Comment,
} from "./api";
import "./style.css";
import { RepoRemove } from "./repo-remove";
import { RepoOpen } from "./repo-open";
import {
  Outgoing,
  QueueDialog,
  LocalDiscussion,
  Marker,
  EventCard,
  pending,
  type Outbox,
  type Event as OutboxEvent,
} from "./outbox";
import { localItems, localDetail, itemTarget, itemNumber, itemEvents } from "./local-items";
const date = (s?: string | null) =>
  s
    ? new Date(s).toLocaleString(undefined, {
        month: "short",
        day: "numeric",
        hour: "2-digit",
        minute: "2-digit",
      })
    : "Never synced";
function CachedImage({ src, alt }: { src?: string; alt?: string }) {
  const [failed, setFailed] = useState(false);
  const [revision, setRevision] = useState(0);
  useEffect(() => setFailed(false), [src]);
  useEffect(() => {
    const retry = () => { setFailed(false); setRevision(value => value + 1); };
    window.addEventListener("ogh-images-pulled", retry);
    return () => window.removeEventListener("ogh-images-pulled", retry);
  }, []);
  if (!src || failed) return <span className="image-placeholder">[Image not cached: {alt || "attached image"}. Pull from GitHub to retry.]</span>;
  return <img className="cached-image" src={`/api/media?${new URLSearchParams({url: src, v: String(revision)})}`} alt={alt || "Comment image"} loading="lazy" onError={() => setFailed(true)} />;
}
// Support GitHub's pasted <img> screenshots while keeping all other HTML disabled.
function imageMarkdown(text: string) {
  return text.replace(/<img\b[^>]*>/gi, tag => {
    const match = /\bsrc\s*=\s*(?:"([^"]+)"|'([^']+)'|([^\s>]+))/i.exec(tag);
    const src = (match?.[1] || match?.[2] || match?.[3] || "").replace(/&amp;/g, "&");
    return /^https:\/\//i.test(src) ? `![Attached image](<${src.replace(/</g, "%3C").replace(/>/g, "%3E")}>)` : "[Image unavailable offline]";
  });
}
function Markdown({ text }: { text?: string | null }) {
  return (
    <div className="markdown">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        skipHtml
        components={{
          img: ({ src, alt }) => <CachedImage src={src} alt={alt} />,
          a: ({ href, children }) => (
            <a href={href} target="_blank" rel="noreferrer">
              {children}
            </a>
          ),
        }}
      >
        {imageMarkdown(text || "_No description provided._")}
      </ReactMarkdown>
    </div>
  );
}
function Empty({
  title,
  children,
}: {
  title: string;
  children: React.ReactNode;
}) {
  return (
    <div className="empty">
      <Inbox size={36} />
      <h2>{title}</h2>
      <p>{children}</p>
    </div>
  );
}
function ErrorBox({ error }: { error: string }) {
  return (
    <div role="alert" className="error">
      <AlertCircle size={17} />
      {error}
    </div>
  );
}
function App() {
  const [initialRoute] = useState(readRoute);
  const [repos, setRepos] = useState<Repo[]>([]),
    [selected, setSelected] = useState(initialRoute.repo),
    [page, setPage] = useState(initialRoute.page),
    [items, setItems] = useState<Item[]>([]),
    [state, setState] = useState("open"),
    [query, setQuery] = useState(""),
    [label, setLabel] = useState(""),
    [labels, setLabels] = useState<string[]>([]),
    [number, setNumber] = useState<number | string | null>(initialRoute.number),
    [detail, setDetail] = useState<Detail | null>(null),
    [loading, setLoading] = useState(true),
    [error, setError] = useState(""),
    [syncing, setSyncing] = useState(false),
    [pulling, setPulling] = useState(false),
    [notice, setNotice] = useState(""),
    [noticeWarning, setNoticeWarning] = useState(false),
    [version, setVersion] = useState(0),
    [addOpen, setAddOpen] = useState(false),
    [newRepo, setNewRepo] = useState(""),
    [adding, setAdding] = useState(false),
    [addError, setAddError] = useState(""),
    [mobile, setMobile] = useState(false);
  const [outbox, setOutbox] = useState<Outbox>({ events: [], counts: {} });
  const [preview, setPreview] = useState<Outbox | null>(null);
  const [previewRepo, setPreviewRepo] = useState("");
  function navigate(route: Route, replace = false) {
    const path = routePath(route);
    if (window.location.pathname !== path) window.history[replace ? "replaceState" : "pushState"](null, "", path);
    setSelected(route.repo); setPage(route.page); setNumber(route.number);
  }
  useEffect(() => {
    const restore = () => { const route = readRoute(); setSelected(route.repo); setPage(route.page); setNumber(route.number); setError(""); };
    window.addEventListener("popstate", restore);
    return () => window.removeEventListener("popstate", restore);
  }, []);
  const refreshOutbox = () => setVersion((v) => v + 1);
  useEffect(() => { setOutbox({ events: [], counts: {} }); }, [selected]);
  useEffect(() => {
    if (!selected) return;
    let current = true;
    const load = () => api<Outbox>(`/api/outbox?${new URLSearchParams({ repo: selected })}`)
      .then((data) => { if (current) setOutbox(data); })
      .catch((e) => { if (current) setError(e.message); });
    load();
    const timer = window.setInterval(load, syncing ? 1500 : 5000);
    return () => { current = false; window.clearInterval(timer); };
  }, [selected, version, syncing]);
  useEffect(() => { setLabel(""); setLabels([]); }, [selected]);
  useEffect(() => { if (!selected) return; let current = true; api<string[]>(repoUrl(selected) + "/labels").then(v => { if (current) setLabels(v); }).catch(e => { if (current) setError(e.message); }); return () => { current = false; }; }, [selected, version]);
  const drafts = localItems(outbox.events, items, repos.find(r => r.full_name === selected)?.last_sync);
  const visibleItems = [...drafts.filter(i => Boolean(i.pull_request) === (page === "pr") &&
    (state === "all" || state === i.state) && (!label || i.labels?.some(l => l.name === label)) &&
    (!query || (numberQuery(query) ? String(i.number) === query.trim().replace(/^(?:#|number:)/, "") : `${i.title} ${i.body}`.toLowerCase().includes(query.toLowerCase())))), ...items];
  const localCreation = typeof number === "string" ? outbox.events.find(e => `local:${e.id}` === number) : undefined;
  const localItem = typeof number === "string" ? localItems(outbox.events, []).find(i => i.local_target === number) : undefined;
  const shownDetail = localCreation?.remote?.number && detail ? detail : localItem ? localDetail(localItem) : detail;
  const outgoingCount = outbox.events.filter(pending).length;
  const repo = repos.find((r) => r.full_name === selected);
  async function loadRepos() {
    const data = await api<Repo[]>("/api/repos");
    setRepos(data);
    const current = readRoute();
    if (data.some(r => r.full_name === current.repo)) {
      navigate(current, true);
    } else if (current.repo && !repos.length && data.length) {
      setError(`Repository ${current.repo} is not in your offline workspace. Add it to open this link.`);
    } else {
      navigate({ repo: data[0]?.full_name || "", page: "issue", number: null }, true);
    }
  }
  useEffect(() => {
    loadRepos()
      .catch((e) => setError(e.message))
      .finally(() => setLoading(false));
  }, []);
  useEffect(() => {
    if (!selected || page === "code" || page === "outgoing") return;
    let current = true;
    setLoading(true);
    setError("");
    const t = setTimeout(() => {
      api<Item[]>(
        `${repoUrl(selected)}/items?${new URLSearchParams({ kind: page, state: numberQuery(query) ? "all" : state, q: query, label })}`,
      )
        .then((d) => {
          if (current) setItems(d);
        })
        .catch((e) => {
          if (current) setError(e.message);
        })
        .finally(() => {
          if (current) setLoading(false);
        });
    }, 200);
    return () => {
      current = false;
      clearTimeout(t);
    };
  }, [selected, page, state, query, label, version]);
  useEffect(() => {
    if (!selected || number === null) {
      setDetail(null);
      return;
    }
    let current = true;
    setLoading(true);
    setError("");
    setDetail(null);
    const remoteNumber = typeof number === "string" ? localCreation?.remote?.number : number;
    if (!remoteNumber || (localCreation && !items.some(i => i.number === remoteNumber))) { setLoading(false); return; }
    api<Detail>(`${repoUrl(selected)}/items/${remoteNumber}`)
      .then((d) => {
        if (current) setDetail(d);
      })
      .catch((e) => {
        if (current) setError(e.message);
      })
      .finally(() => {
        if (current) setLoading(false);
      });
    return () => {
      current = false;
    };
  }, [selected, number, version, localCreation?.remote?.number, items]);
  async function openSearchNumber() {
    if (!numberQuery(query) || !selected) return;
    const target = Number(query.trim().replace(/^(?:#|number:)/, ""));
    try {
      const result = await api<Detail>(`${repoUrl(selected)}/items/${target}`);
      navigate({ repo: selected, page: result.item.pull_request ? "pr" : "issue", number: target });
    } catch (error) { setError((error as Error).message); }
  }
  async function pull() {
    if (!selected) return;
    setPulling(true); setError(""); setNotice(""); setNoticeWarning(false);
    try {
      const result = await api<{ images?: { downloaded?: number; failed?: number; deferred?: number; error?: string } }>(repoUrl(selected) + "/sync", { method: "POST", body: "{}" });
      window.dispatchEvent(new Event("ogh-images-pulled"));
      const images = result.images;
      const imageNotice = images ? images.error ? " Image caching failed; retry Pull." : ` Images: ${images.downloaded || 0} downloaded, ${images.failed || 0} unavailable${images.deferred ? `, ${images.deferred} waiting for the next Pull` : ""}.` : "";
      setNotice("Pull completed. Your local copy is up to date; outgoing changes were not sent." + imageNotice);
      await loadRepos(); refreshOutbox();
    } catch (error) { setError((error as Error).message); }
    finally { setPulling(false); }
  }
  async function sync() {
    if (!selected) return;
    setSyncing(true);
    setError("");
    setNotice("");
    setNoticeWarning(false);
    try {
      const plan = await api<Outbox>("/api/outbox/sync", {
        method: "POST",
        body: JSON.stringify({ repo: selected, dry_run: true }),
      });
      setPreviewRepo(selected);
      setPreview(plan);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setSyncing(false);
    }
  }
  async function executeSync() {
    setSyncing(true);
    setError("");
    setNotice("");
    setNoticeWarning(false);
    try {
      const result = await api<Outbox>("/api/outbox/sync", {
        method: "POST",
        body: JSON.stringify({ repo: previewRepo, plan_token: preview?.plan_token, pull: false }),
      });
      const remaining = result.events.filter(pending).length;
      setNoticeWarning(remaining > 0);
      setNotice(remaining ? `${remaining} outgoing changes remain unresolved.` : "Push completed. Outgoing changes reconciled. Pull separately to download GitHub updates.");
      setPreview(null);
      await loadRepos();
      refreshOutbox();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setSyncing(false);
    }
  }
  async function add(e: React.FormEvent) {
    e.preventDefault();
    setAddError("");
    if (!/^[\w.-]+\/[\w.-]+$/.test(newRepo.trim())) {
      setAddError("Enter a repository as owner/repository.");
      return;
    }
    setAdding(true);
    try {
      const r = await api<Repo>("/api/repos", {
        method: "POST",
        body: JSON.stringify({ full_name: newRepo.trim() }),
      });
      await loadRepos();
      navigate({ repo: r.full_name, page: "issue", number: null });
      setAddOpen(false);
      setNewRepo("");
      setNoticeWarning(false);
      setNotice("Repository added. Pull from GitHub to download a local copy.");
    } catch (e) {
      setAddError((e as Error).message);
    } finally {
      setAdding(false);
    }
  }
  return (
    <div className="app">
      <aside className={mobile ? "sidebar mobile-open" : "sidebar"}>
        <a className="brand" href="#" onClick={(e) => e.preventDefault()}>
          <div className="brand-icon">
            <img src="/brand/mark.svg" alt="" />
          </div>
          <span>
            offline<span className="brand-light">github</span>
            <small>YOUR WORK. ANYWHERE.</small>
          </span>
        </a>
        <div className="workspace-label">
          WORKSPACE <span>LOCAL</span>
        </div>
        <div className="projects-heading">
          Repositories{" "}
          <Button
            variant="ghost"
            aria-label="Add repository"
            onClick={() => setAddOpen(true)}
          >
            <Plus size={17} />
          </Button>
        </div>
        <nav aria-label="Repositories">
          {repos.map((r) => (
            <button
              key={r.full_name}
              className={
                "repo-link " + (selected === r.full_name ? "active" : "")
              }
              onClick={() => {
                navigate({ repo: r.full_name, page, number: null });
                setError("");
                setNotice("");
    setNoticeWarning(false);
                setMobile(false);
              }}
            >
              <span className="repo-avatar">
                {r.full_name.split("/")[1].slice(0, 2).toUpperCase()}
              </span>
              <span>
                <strong>{r.full_name.split("/")[1]}</strong>
                <small>{r.full_name.split("/")[0]}</small>
              </span>
              {selected === r.full_name && <span className="active-dot" />}
            </button>
          ))}
        </nav>
        <Button
          className="add-repo"
          variant="ghost"
          onClick={() => setAddOpen(true)}
        >
          <Plus size={16} /> Add repository
        </Button>
        <div className="sidebar-bottom">
          <div className="offline-badge">
            <WifiOff size={17} />
            <span>Built for offline</span>
            <span className="green-dot" />
          </div>
          <p>
            Your local archive. GitHub stays
            <br />
            the source of truth.
          </p>
          <span className="version">
            LOCAL-FIRST WORKSPACE <span>v0.2</span>
          </span>
        </div>
      </aside>
      <div className="main">
        <header>
          <div className="breadcrumb">
            <Button
              variant="ghost"
              className="mobile-toggle"
              aria-label="Toggle repositories"
              onClick={() => setMobile(!mobile)}
            >
              <Menu size={20} />
            </Button>
            <img src="/brand/mark.svg" alt="" />
            <span>{selected.split("/")[0] || "Workspace"}</span>
            <span className="slash">/</span>
            <strong>{selected.split("/")[1] || "Overview"}</strong>
            <span className="local-tag">Local copy</span>
          </div>
          <span className="readonly">
            <span className="green-dot" /> Local queue
          </span>
        </header>
        <main>
          <div className="page-heading">
            <div>
              <div className="eyebrow">YOUR LOCAL WORKSPACE</div>
              <h1>{repo?.full_name.split("/")[1] || "Welcome aboard."}</h1>
              <p>
                {repo?.description ||
                  "Keep your repositories close, wherever work takes you."}
              </p>
            </div>
            <div className="sync-group">
              <div className="transfer-buttons">
                <Button variant="outline" onClick={pull} disabled={!selected || syncing || pulling}>
                  <ArrowDownToLine size={15} className={pulling ? "spin" : ""} />
                  {pulling ? "Pulling…" : "Pull from GitHub"}
                </Button>
                <Button onClick={sync} disabled={!selected || syncing || pulling}>
                  <ArrowUpFromLine size={15} />
                  {syncing ? "Preparing push…" : "Push changes"}
                </Button>
              </div>
              <small>{pulling ? "Checking GitHub for incoming changes…" : `Last pulled ${date(repo?.last_sync)}`}</small>
            </div>
          </div>
          {selected && <RepoOpen key={selected} repo={selected} version={version}><RepoRemove key={"remove-"+selected} repo={selected} onRemoved={async () => { setNumber(null); setItems([]); await loadRepos(); refreshOutbox(); }} /></RepoOpen>}
          {error && <ErrorBox error={error} />}
          {notice && (
            <div className={noticeWarning ? "notice notice-warning" : "notice"} role={noticeWarning ? "alert" : "status"}>
              {noticeWarning ? <AlertCircle size={16} /> : <Check size={16} />}
              {notice}
            </div>
          )}
          {!selected ? (
            <Empty
              title={
                loading ? "Loading workspace…" : "Your workspace starts here"
              }
            >
              Add a GitHub repository, then pull to take issues, pull requests
              and code with you.
              <br />
              <Button variant="default" onClick={() => setAddOpen(true)}>
                <Plus size={16} /> Add your first repository
              </Button>
            </Empty>
          ) : (
            <>
              <div
                className="section-tabs"
                role="navigation"
                aria-label="Repository views"
              >
                {[
                  ["issue", "Issues", CircleDot, repo?.issue_count],
                  ["pr", "Pull requests", GitPullRequest, repo?.pr_count],
                  ["code", "Code", Code2, null],
                  ["outgoing", "Outgoing", Inbox, outgoingCount],
                ].map(([id, label, Icon, count]) => {
                  const I = Icon as typeof CircleDot;
                  return (
                    <button
                      key={String(id)}
                      className={page === id ? "selected" : ""}
                      onClick={() => {
                        navigate({ repo: selected, page: String(id), number: null });
                        setError("");
                      }}
                    >
                      <I size={17} />
                      {String(label)}
                      {count !== null && (
                        <span className="count">{count as number}</span>
                      )}
                    </button>
                  );
                })}
                <span className="cache-note">
                  <WifiOff size={13} /> Available offline after pull
                </span>
              </div>
              {page === "outgoing" ? (
                <Outgoing
                  data={outbox}
                  repo={selected}
                  refresh={refreshOutbox}
                />
              ) : page === "code" ? (
                <CodeBrowser repo={selected} version={version} />
              ) : number !== null ? (
                <>
                  {
                    <Button variant="ghost" onClick={() => navigate({ repo: selected, page, number: null })}>
                      <ArrowLeft size={15} /> Back to{" "}
                      {page === "pr" ? "pull requests" : "issues"}
                    </Button>
                  }
                  {loading ? (
                    <div className="loading">Loading discussion…</div>
                  ) : (
                    shownDetail && (
                      <ItemDetail
                        key={String(number)}
                        detail={shownDetail}
                        repo={selected}
                        isPr={page === "pr"}
                        events={itemEvents(shownDetail.item, outbox.events)}
                        refresh={refreshOutbox}
                      />
                    )
                  )}
                </>
              ) : (
                <>
                  <div className="queue-actions">
                    <QueueDialog
                      repo={selected}
                      kind="issue_create"
                      refresh={refreshOutbox}
                    >
                      New issue
                    </QueueDialog>
                    <QueueDialog
                      repo={selected}
                      kind="label_create"
                      refresh={refreshOutbox}
                    >
                      New label
                    </QueueDialog>
                    <QueueDialog
                      repo={selected}
                      kind="pr_create"
                      refresh={refreshOutbox}
                    >
                      New pull request
                    </QueueDialog>
                  </div>
                  <div className="list-toolbar">
                    <div className="state-control">
                      {["open", "closed", "all"].map((s) => (
                        <button
                          key={s}
                          className={state === s ? "selected" : ""}
                          onClick={() => setState(s)}
                        >
                          {s[0].toUpperCase() + s.slice(1)}
                        </button>
                      ))}
                    </div>
                    <select className="label-filter" aria-label="Filter by label" value={label} onChange={e => setLabel(e.target.value)}><option value="">All labels</option>{[...new Set([...labels, ...drafts.flatMap(i => i.labels?.map(l => l.name) || [])])].sort().map(name => <option key={name} value={name}>{name}</option>)}</select>
                    <label className="search">
                      <Search size={16} />
                      <input
                        value={query}
                        onChange={(e) => setQuery(e.target.value)}
                        onKeyDown={(e) => { if (e.key === "Enter") { e.preventDefault(); void openSearchNumber(); } }}
                        placeholder={`Search ${page === "pr" ? "pull requests" : "issues"} or #number…`}
                        aria-label="Search cached items"
                      />
                      {query && (
                        <button
                          aria-label="Clear search"
                          onClick={() => setQuery("")}
                        >
                          <X size={14} />
                        </button>
                      )}
                    </label>
                  </div>
                  <div className="item-list">
                    {loading ? (
                      <div className="loading">Loading your local cache…</div>
                    ) : visibleItems.length === 0 ? (
                      <Empty
                        title={
                          query ? "No matching results" : "Nothing here yet"
                        }
                      >
                        {query
                          ? "Try a different search or status."
                          : "Pull from GitHub to download its latest issues and pull requests."}
                      </Empty>
                    ) : (
                      visibleItems.map((i) => (
                        <button
                          className="item-row"
                          key={itemTarget(i)}
                          onClick={() => navigate({ repo: selected, page: i.pull_request ? "pr" : "issue", number: i.local_target || i.number })}
                        >
                          {page === "pr" ? (
                            <GitPullRequest
                              className={
                                i.merged_at
                                  ? "purple"
                                  : i.state === "open"
                                    ? "green"
                                    : "muted"
                              }
                              size={20}
                            />
                          ) : (
                            <CircleDot
                              className={
                                i.state === "open" ? "green" : "purple"
                              }
                              size={20}
                            />
                          )}
                          <div className="item-text">
                            <div className="item-title">
                              {i.title}{" "}
                              {outbox.events
                                .filter(
                                  (e) =>
                                    (e.target === itemTarget(i) || e.id === i.local_target?.slice(6)) && pending(e),
                                )
                                .map((e) => (
                                  <Marker key={e.id} event={e} />
                                ))}
                              {i.labels?.map((l) => (
                                <span
                                  className="label"
                                  key={l.name}
                                  style={{
                                    borderColor: "#" + l.color + "66",
                                    color: "#" + l.color,
                                  }}
                                >
                                  {l.name}
                                </span>
                              ))}
                            </div>
                            <small>
                              {itemNumber(i)} · {i.user?.login || "Unknown author"}{" "}
                              · updated {date(i.updated_at)}
                            </small>
                          </div>
                          {i.comments > 0 && (
                            <span className="comments-count">
                              <MessageSquare size={14} />
                              {i.comments}
                            </span>
                          )}
                          <ChevronRight size={16} className="row-arrow" />
                        </button>
                      ))
                    )}
                  </div>
                  <div className="list-footer">
                    <span>
                      {visibleItems.length} local and cached{" "}
                      {page === "pr" ? "pull requests" : "issues"}
                    </span>
                    <span>Pull downloads updates. Push sends reviewed local changes.</span>
                  </div>
                </>
              )}
            </>
          )}
          <footer>
            <span>
              <img src="/brand/mark.svg" width={16} height={16} alt="" />{" "}
              Offline GitHub
            </span>
            <span>A little less connected. Just as productive.</span>
          </footer>
        </main>
      </div>
      <Dialog.Root
        open={preview !== null}
        onOpenChange={(v) => {
          if (!v && !syncing) setPreview(null);
        }}
      >
        <Dialog.Portal>
          <Dialog.Overlay className="dialog-overlay" />
          <Dialog.Content className="dialog-content queue-dialog">
            <Dialog.Title>Push preview · {previewRepo}</Dialog.Title>
            <Dialog.Description>
              Review saved changes below. Confirming sends queued work to
              GitHub. Incoming changes are downloaded separately with Pull. Conflicts remain queued
              for your decision.
            </Dialog.Description>
            {preview?.events.filter(pending).map((e) => (
              <EventCard
                key={e.id}
                event={e}
                refresh={() => {
                  setPreview(null);
                  refreshOutbox();
                }}
              />
            ))}
            {!preview?.events.some(pending) && (
              <p>
                No outgoing changes to push. Use Pull from GitHub to check for incoming changes.
              </p>
            )}
            {error && <ErrorBox error={error} />}
            <div className="queue-actions">
              <Button disabled={syncing} onClick={() => setPreview(null)}>
                Cancel
              </Button>
              <Button disabled={syncing || !preview?.events.some(pending)} onClick={executeSync}>
                {syncing
                  ? "Pushing…"
                  : `Push ${preview?.events.filter(pending).length || 0} outgoing changes`}
              </Button>
            </div>
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog.Root>
      <Dialog.Root open={addOpen} onOpenChange={setAddOpen}>
        <Dialog.Portal>
          <Dialog.Overlay className="dialog-overlay" />
          <Dialog.Content className="dialog-content">
            <Dialog.Title>Add a repository</Dialog.Title>
            <Dialog.Description>
              Bring your GitHub work into your local workspace. Adding a
              repository does not start a sync.
            </Dialog.Description>
            <Dialog.Close asChild>
              <Button
                variant="ghost"
                className="dialog-close"
                aria-label="Close"
              >
                <X size={18} />
              </Button>
            </Dialog.Close>
            <form onSubmit={add}>
              <label htmlFor="repository">GitHub repository</label>
              <input
                id="repository"
                autoFocus
                value={newRepo}
                onChange={(e) => setNewRepo(e.target.value)}
                placeholder="owner/repository"
              />
              {addError && <ErrorBox error={addError} />}
              <Button variant="default" disabled={adding} type="submit">
                {adding ? "Adding…" : "Add repository"}
                <Plus size={16} />
              </Button>
            </form>
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog.Root>
    </div>
  );
}
function ItemDetail({
  detail: d,
  repo,
  isPr,
  events,
  refresh,
}: {
  detail: Detail;
  repo: string;
  isPr: boolean;
  events: OutboxEvent[];
  refresh: () => void;
}) {
  const [copied, setCopied] = useState(false),
    [copyError, setCopyError] = useState("");
  const cmd = `ogh pr checkout ${d.item.number} -R ${repo} --dir ../${repo.split("/")[1]}-pr-${d.item.number}`;
  async function copy() {
    try {
      await navigator.clipboard.writeText(cmd);
      setCopied(true);
      setCopyError("");
    } catch {
      setCopyError("Clipboard unavailable. Select and copy the command below.");
    }
  }
  return (
    <article className="detail">
      <h2>
        {d.item.title} <span>{itemNumber(d.item)}</span>
      </h2>
      {typeof d.item.number === "string" && <p className="muted">Local draft · saved in Outgoing. GitHub assigns a number after Push.</p>}
      <div className="detail-meta">
        <span className={"status " + (d.item.merged_at ? "merged" : "")}>
          {d.item.merged_at ? "Merged" : d.item.draft ? "Draft" : d.item.state}
        </span>
        <span>
          {d.item.user?.login} opened {date(d.item.created_at)}
        </span>
        {isPr && d.item.head && (
          <code>
            {d.item.head.ref} → {d.item.base?.ref}
          </code>
        )}
      </div>
      <div className="queue-actions">
        {events.filter(e => ["issue_create", "pr_create"].includes(e.kind)).map(e => <Marker key={e.id} event={e} />)}
        <QueueDialog
          repo={repo}
          kind="edit"
          target={itemTarget(d.item)}
          initial={{
            title: d.item.title,
            body: d.item.body || "",
            item_kind: isPr ? "pr" : "issue",
          }}
          refresh={refresh}
        >
          Edit title and body
        </QueueDialog>
        <QueueDialog
          repo={repo}
          kind="labels"
          target={itemTarget(d.item)}
          refresh={refresh}
        >
          Add / remove labels
        </QueueDialog>
      </div>
      <div className="queue-actions">
        {d.item.labels?.map((l) => (
          <span className="label" key={l.name}>
            {l.name}
          </span>
        ))}
      </div>
      {events
        .filter((e) => ["edit", "labels"].includes(e.kind) && pending(e))
        .map((e) => (
          <div className="queued-field" key={e.id}>
            <strong>
              {e.kind === "edit" ? "Title / body change" : "Label change"}
            </strong>{" "}
            <Marker event={e} />
            <pre>{JSON.stringify(e.payload, null, 2)}</pre>
          </div>
        ))}
      {isPr && typeof d.item.number === "number" && (
        <div className="checkout">
          <div>
            <GitBranch size={17} />
            <strong>Test this pull request locally</strong>
            <Button onClick={copy}>
              <>
                {copied ? <Check size={14} /> : <Copy size={14} />}{" "}
                {copied ? "Copied" : "Copy command"}
              </>
            </Button>
          </div>
          <code>{cmd}</code>
          <small>
            Creates an isolated worktree. Your current checkout stays in place.
          </small>
          {copyError && <ErrorBox error={copyError} />}
        </div>
      )}
      <Tabs.Root defaultValue="discussion">
        <Tabs.List className="detail-tabs" aria-label="Item details">
          <Tabs.Trigger value="discussion">
            Discussion <span className="count">{d.comments.length}</span>
          </Tabs.Trigger>
          {isPr && (
            <>
              <Tabs.Trigger value="reviews">
                Reviews <span className="count">{d.reviews.length}</span>
              </Tabs.Trigger>
              <Tabs.Trigger value="files">
                Files changed <span className="count">{d.files.length}</span>
              </Tabs.Trigger>
              <Tabs.Trigger value="diff">Unified diff</Tabs.Trigger>
            </>
          )}
        </Tabs.List>
        <Tabs.Content value="discussion">
          <div className="comment">
            <div className="comment-header">
              <strong>{d.item.user?.login}</strong>
              <time>{date(d.item.created_at)}</time>
            </div>
            <Markdown text={d.item.body} />
          </div>
          {d.comments.map((c) => (
            <CommentCard key={c.id} c={c} />
          ))}
          <LocalDiscussion
            events={events}
            detail={d}
            repo={repo}
            refresh={refresh}
          />
        </Tabs.Content>
        <Tabs.Content value="reviews">
          {d.reviews_cached === false && d.reviews.length > 0 && (
            <p className="muted">
              Review summaries may be stale. Run ogh sync -R {repo} --reviews to
              refresh them.
            </p>
          )}
          {!d.reviews.length && !d.review_comments.length && (
            <Empty title="No reviews cached">
              Run ogh sync -R {repo} --reviews to download review summaries.
            </Empty>
          )}
          {d.reviews.map((c) => (
            <CommentCard key={c.id} c={c} />
          ))}
          {d.review_comments.map((c) => (
            <CommentCard key={c.id} c={c} />
          ))}
        </Tabs.Content>
        <Tabs.Content value="files">
          {!d.files.length && (
            <Empty title="No changed files cached">
              Sync to download pull request files.
            </Empty>
          )}
          {d.files.map((f) => (
            <div className="file-card" key={f.filename}>
              <div className="file-heading">
                <FileText size={15} />
                <code>{f.filename}</code>
                <span>{f.status}</span>
                <b className="green">
                  {f.additions == null ? "binary" : `+${f.additions}`}
                </b>
                <b className="red">
                  {f.deletions == null ? "" : `−${f.deletions}`}
                </b>
              </div>
              <Diff
                text={
                  f.patch ||
                  "Patch unavailable for this file (it may be binary or too large)."
                }
              />
            </div>
          ))}
        </Tabs.Content>
        <Tabs.Content value="diff">
          {d.diff_error && <ErrorBox error={d.diff_error} />}
          {d.diff_truncated && (
            <p className="muted">
              Preview limited to 4 MB. Use ogh pr diff for the full patch.
            </p>
          )}
          <Diff
            text={
              d.diff ||
              "No unified diff cached. Sync this repository to download it."
            }
          />
        </Tabs.Content>
      </Tabs.Root>
    </article>
  );
}
function CommentCard({ c }: { c: Comment }) {
  return (
    <div className="comment">
      <div className="comment-header">
        <strong>{c.user?.login || "Unknown author"}</strong>
        {c.state && (
          <span className="review-state">{c.state.replaceAll("_", " ")}</span>
        )}
        <time>{date(c.created_at || c.submitted_at)}</time>
      </div>
      {c.path && (
        <div className="comment-path">
          <code>
            {c.path}
            {c.line ? `:${c.line}` : ""}
          </code>
        </div>
      )}
      <Markdown text={c.body} />
    </div>
  );
}
function Diff({ text }: { text: string }) {
  return (
    <pre className="diff">
      <code>
        {text.split("\n").map((line, i) => (
          <span
            key={i}
            className={
              line.startsWith("+")
                ? "diff-add"
                : line.startsWith("-")
                  ? "diff-remove"
                  : line.startsWith("@@")
                    ? "diff-hunk"
                    : ""
            }
          >
            {line || " "}
            <br />
          </span>
        ))}
      </code>
    </pre>
  );
}
function CodeBrowser({ repo, version }: { repo: string; version: number }) {
  const [path, setPath] = useState(""),
    [ref, setRef] = useState("HEAD"),
    [refInput, setRefInput] = useState("HEAD"),
    [entries, setEntries] = useState<Entry[]>([]),
    [file, setFile] = useState<string | null>(null),
    [content, setContent] = useState(""),
    [error, setError] = useState(""),
    [loading, setLoading] = useState(false);
  useEffect(() => {
    setPath("");
    setFile(null);
    setRef("HEAD");
    setRefInput("HEAD");
  }, [repo]);
  useEffect(() => {
    let current = true;
    setLoading(true);
    setError("");
    const params = new URLSearchParams({ ref, path: file ?? path });
    api<Entry[] | { content: string }>(
      `${repoUrl(repo)}/${file === null ? "tree" : "blob"}?${params}`,
    )
      .then((d) => {
        if (!current) return;
        if (Array.isArray(d)) setEntries(d);
        else setContent(d.content);
      })
      .catch((e) => {
        if (current) setError(e.message);
      })
      .finally(() => {
        if (current) setLoading(false);
      });
    return () => {
      current = false;
    };
  }, [repo, path, file, ref, version]);
  return (
    <div className="code-browser">
      <div className="code-toolbar">
        <form
          onSubmit={(e) => {
            e.preventDefault();
            setRef(refInput);
            setFile(null);
          }}
        >
          <GitBranch size={15} />
          <input
            aria-label="Git ref"
            value={refInput}
            onChange={(e) => setRefInput(e.target.value)}
          />
          <Button type="submit">Browse ref</Button>
        </form>
        <span className="muted">Local git mirror</span>
      </div>
      <div className="path">
        <button
          onClick={() => {
            setPath("");
            setFile(null);
          }}
        >
          {repo.split("/")[1]}
        </button>
        {(file ?? path)
          .split("/")
          .filter(Boolean)
          .map((p, i, parts) => (
            <React.Fragment key={i}>
              <ChevronRight size={13} />
              <button
                onClick={() => {
                  if (file !== null && i === parts.length - 1) return;
                  setPath(parts.slice(0, i + 1).join("/"));
                  setFile(null);
                }}
              >
                {p}
              </button>
            </React.Fragment>
          ))}
      </div>
      {error ? (
        <ErrorBox error={error} />
      ) : loading ? (
        <div className="loading">Reading local files…</div>
      ) : file !== null ? (
        <div className="file-card">
          <div className="file-heading">
            <FileText size={15} />
            {file}
            <span>{content.split("\n").length} lines</span>
          </div>
          <pre className="source">
            <code>{content}</code>
          </pre>
        </div>
      ) : (
        <div className="file-tree">
          {path && (
            <button
              onClick={() => setPath(path.split("/").slice(0, -1).join("/"))}
            >
              <ArrowLeft size={16} />
              ..
            </button>
          )}
          {[...entries]
            .sort((a, b) =>
              a.kind === b.kind
                ? a.name.localeCompare(b.name)
                : a.kind === "tree"
                  ? -1
                  : 1,
            )
            .map((e) => (
              <button
                key={e.path}
                onClick={() =>
                  e.kind === "tree" ? setPath(e.path) : setFile(e.path)
                }
              >
                {e.kind === "tree" ? (
                  <Folder size={17} className="folder" />
                ) : (
                  <FileText size={17} />
                )}
                <span>{e.name}</span>
                <ChevronRight size={15} />
              </button>
            ))}
          {entries.length === 0 && (
            <Empty title="No files available">
              Sync the repository to download its git mirror.
            </Empty>
          )}
        </div>
      )}
    </div>
  );
}
createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);

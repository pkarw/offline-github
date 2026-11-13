import React, { useState } from "react";
import * as Dialog from "@radix-ui/react-dialog";
import { api, type Detail } from "./api";
import { Button } from "./ui";
export type Event = {
  id: string;
  seq: number;
  repo: string;
  kind: string;
  target: string | null;
  payload: Record<string, unknown>;
  base: unknown;
  observed: unknown;
  status: string;
  error: string | null;
  remote: { id?: number } | null;
  history: { at: string; action: string; data: unknown }[];
  created_at: string;
};
export type Outbox = {
  events: Event[];
  counts: Record<string, number>;
  incoming_errors?: unknown;
  plan_token?: string;
};
export const pending = (e: Event) =>
  !["applied", "cancelled"].includes(e.status);
export function Marker({ event }: { event: Event }) {
  return (
    <span className={`queue-marker queue-${event.status}`}>
      {event.status === "queued" ? "pending" : event.status}
    </span>
  );
}
export function EventCard({
  event: e,
  refresh,
}: {
  event: Event;
  refresh: () => void;
}) {
  const [error, setError] = useState(""),
    [busy, setBusy] = useState(false);
  async function resolve(action: string) {
    setBusy(true);
    setError("");
    try {
      await api(`/api/outbox/${encodeURIComponent(e.id)}/resolve`, {
        method: "POST",
        body: JSON.stringify({ action }),
      });
      refresh();
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setBusy(false);
    }
  }
  return (
    <article className="queue-card">
      <div className="queue-heading">
        <strong>
          #{e.seq} {e.kind.replaceAll("_", " ")} {e.target && `· #${e.target}`}
        </strong>
        <Marker event={e} />
      </div>
      <small>
        {e.repo} · {new Date(e.created_at).toLocaleString()}
      </small>
      {e.error && (
        <p role="alert" className="red">
          {e.error}
        </p>
      )}
      <details open={e.status === "conflict"}>
        <summary>Change details and history</summary>
        <div className="conflict-grid">
          {[
            ["Base", e.base],
            ["Local", e.payload],
            ["Observed on GitHub", e.observed],
          ].map(([title, value]) => (
            <section key={String(title)}>
              <h4>{String(title)}</h4>
              <pre>{JSON.stringify(value, null, 2) ?? "Not available"}</pre>
            </section>
          ))}
        </div>
        <h4>Event history</h4>
        {e.history.map((h, i) => (
          <div className="audit-row" key={i}>
            <time>{h.at}</time> {h.action}
            <pre>{JSON.stringify(h.data, null, 2)}</pre>
          </div>
        ))}
      </details>
      <div className="queue-actions">
        {e.status === "conflict" && (
          <>
            <Button disabled={busy} onClick={() => resolve("use_local")}>
              Use local change
            </Button>
            <Button disabled={busy} onClick={() => resolve("keep_remote")}>
              Keep GitHub version
            </Button>
          </>
        )}
        {["queued", "failed"].includes(e.status) && <Button disabled={busy} onClick={() => resolve("keep_remote")}>Cancel queued change</Button>}
        {["failed", "uncertain"].includes(e.status) && (
          <Button disabled={busy} onClick={() => resolve("retry")}>
            Retry safely
          </Button>
        )}
      </div>
      {error && (
        <p role="alert" className="red">
          {error}
        </p>
      )}
    </article>
  );
}
export function Outgoing({
  data,
  refresh,
  repo,
}: {
  data: Outbox;
  refresh: () => void;
  repo: string;
}) {
  const [filter, setFilter] = useState("all"),
    [error, setError] = useState("");
  async function download(format: "json" | "md") {
    try {
      const r = await fetch(
        `/api/outbox?${new URLSearchParams({ repo, format })}`,
      );
      if (!r.ok) throw new Error(`Report failed (${r.status})`);
      const blob = await r.blob();
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `outgoing-${repo.replaceAll("/", "-")}.${format}`;
      a.click();
      URL.revokeObjectURL(url);
    } catch (e) {
      setError((e as Error).message);
    }
  }
  return (
    <section>
      <div className="queue-actions">
        <h2>Outgoing changes</h2>
        <Button onClick={() => download("json")}>Download JSON</Button>
        <Button onClick={() => download("md")}>Download Markdown</Button>
      </div>
      <p className="muted">
        Changes are saved locally. Only an explicit sync sends them to GitHub.
        Reports include local bodies and conflict history.
      </p>
      <label>
        Status{" "}
        <select value={filter} onChange={(e) => setFilter(e.target.value)}>
          {[
            "all",
            "queued",
            "sending",
            "conflict",
            "failed",
            "uncertain",
            "applied",
            "cancelled",
          ].map((s) => (
            <option key={s}>{s}</option>
          ))}
        </select>
      </label>
      {error && <p role="alert">{error}</p>}
      {data.events
        .filter((e) => filter === "all" || e.status === filter)
        .map((e) => (
          <EventCard key={e.id} event={e} refresh={refresh} />
        ))}
      {!data.events.some((e) => filter === "all" || e.status === filter) && (
        <p className="muted">No matching outgoing events.</p>
      )}
    </section>
  );
}
export function QueueDialog({
  repo,
  kind,
  target,
  initial,
  refresh,
  children,
}: {
  repo: string;
  kind: string;
  target?: string;
  initial?: Record<string, string>;
  refresh: () => void;
  children: React.ReactNode;
}) {
  const [open, setOpen] = useState(false),
    [values, setValues] = useState<Record<string, string>>({}),
    [busy, setBusy] = useState(false),
    [error, setError] = useState("");
  const fields =
    kind === "label_create"
      ? ["name", "color", "description"]
      : kind === "labels"
        ? ["add", "remove"]
        : kind === "pr_create"
          ? ["title", "body", "head", "base"]
          : ["title", "body"];
  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError("");
    const list = (s?: string) =>
      (s || "")
        .split(",")
        .map((v) => v.trim())
        .filter(Boolean);
    let payload: unknown = values;
    if (kind === "edit")
      payload = {
        item_kind: initial?.item_kind || "issue",
        fields: { title: values.title, body: values.body },
      };
    if (kind === "labels")
      payload = { add: list(values.add), remove: list(values.remove) };
    try {
      await api("/api/outbox", {
        method: "POST",
        body: JSON.stringify({ repo, kind, target, payload }),
      });
      refresh();
      setOpen(false);
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setBusy(false);
    }
  }
  return (
    <Dialog.Root
      open={open}
      onOpenChange={(v) => {
        setOpen(v);
        if (v) {
          setValues(
            initial || (kind === "label_create" ? { color: "808080" } : {}),
          );
          setError("");
        }
      }}
    >
      <Dialog.Trigger asChild>
        <Button>{children}</Button>
      </Dialog.Trigger>
      <Dialog.Portal>
        <Dialog.Overlay className="dialog-overlay" />
        <Dialog.Content className="dialog-content queue-dialog">
          <Dialog.Title>{String(children)}</Dialog.Title>
          <Dialog.Description>
            Save an intention locally. GitHub changes only when you explicitly
            sync.{kind === "labels" && " Enter comma-separated label names."}
          </Dialog.Description>
          <form onSubmit={submit}>
            {fields.map((f) => (
              <label key={f}>
                {f[0].toUpperCase() + f.slice(1)}
                {f === "body" || f === "description" ? (
                  <textarea
                    value={values[f] || ""}
                    onChange={(e) =>
                      setValues({ ...values, [f]: e.target.value })
                    }
                  />
                ) : (
                  <input
                    required={[
                      "title",
                      "name",
                      "head",
                      "base",
                      "color",
                    ].includes(f)}
                    pattern={f === "color" ? "[0-9a-fA-F]{6}" : undefined}
                    value={values[f] || ""}
                    onChange={(e) =>
                      setValues({ ...values, [f]: e.target.value })
                    }
                  />
                )}
              </label>
            ))}
            {error && (
              <p role="alert" className="red">
                {error}
              </p>
            )}
            <div className="queue-actions">
              <Dialog.Close asChild>
                <Button type="button">Cancel</Button>
              </Dialog.Close>
              <Button type="submit" disabled={busy}>
                {busy ? "Saving…" : "Queue change"}
              </Button>
            </div>
          </form>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
export function LocalDiscussion({
  events,
  detail,
  repo,
  refresh,
}: {
  events: Event[];
  detail: Detail;
  repo: string;
  refresh: () => void;
}) {
  const [body, setBody] = useState(""),
    [error, setError] = useState(""),
    [busy, setBusy] = useState(false);
  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError("");
    try {
      await api("/api/outbox", {
        method: "POST",
        body: JSON.stringify({
          repo,
          kind: "comment",
          target: String(detail.item.number),
          payload: { body },
        }),
      });
      setBody("");
      refresh();
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setBusy(false);
    }
  }
  return (
    <>
      {events
        .filter(
          (e) =>
            e.kind === "comment" &&
            e.status !== "cancelled" &&
            !(
              e.status === "applied" &&
              e.remote?.id &&
              detail.comments.some((c) => c.id === e.remote?.id)
            ),
        )
        .map((e) => (
          <article className="comment local-comment" key={e.id}>
            <div className="comment-header">
              <strong>Local comment</strong>
              <Marker event={e} />
            </div>
            <div className="local-body">{String(e.payload.body || "")}</div>
            {e.error && <p className="red">{e.error}</p>}
          </article>
        ))}
      <form className="composer" onSubmit={submit}>
        <label htmlFor="local-comment">Add a comment to the local queue</label>
        <textarea
          id="local-comment"
          required
          value={body}
          onChange={(e) => setBody(e.target.value)}
          placeholder="Write Markdown…"
        />
        <Button disabled={busy || !body.trim()} type="submit">
          {busy ? "Saving…" : "Queue comment"}
        </Button>
        {error && (
          <p role="alert" className="red">
            {error}
          </p>
        )}
      </form>
    </>
  );
}

import React, { useEffect, useRef, useState } from "react";
import { ChevronDown, Folder, Terminal, RefreshCw } from "lucide-react";
import { api, repoUrl } from "./api";

type Context = { path: string; workspaces: string[]; checkout_exists: boolean; tools: { id: string; label: string; available: boolean; reason: string }[] };
export function RepoOpen({ repo, version }: { repo: string; version: number }) {
  const [context, setContext] = useState<Context>();
  const [path, setPath] = useState("");
  const [message, setMessage] = useState("");
  const [busy, setBusy] = useState(false);
  const [refresh, setRefresh] = useState(0);
  const dropdown = useRef<HTMLDetailsElement>(null);
  useEffect(() => {
    let active = true;
    api<Context>(repoUrl(repo) + "/open").then(data => {
      if (active) { setContext(data); setPath(old => data.workspaces.includes(old) ? old : data.path); }
    }).catch(error => { if (active) setMessage(String(error)); });
    return () => { active = false; };
  }, [repo, version, refresh]);
  async function launch(tool: string) {
    if (dropdown.current) dropdown.current.open = false;
    setBusy(true); setMessage("");
    try {
      const result = await api<{ message: string }>(repoUrl(repo) + "/open", { method: "POST", body: JSON.stringify({ tool, path }) });
      setMessage(result.message); setRefresh(n => n + 1);
    } catch (error) { setMessage(String(error)); }
    finally { setBusy(false); }
  }
  return <div className="repo-launcher">
    <div className="checkout-path"><Folder size={16} /><div><small>Local checkout</small>
      {context && context.workspaces.length > 1 ? <select aria-label="Local checkout" value={path} onChange={e => setPath(e.target.value)}>{context.workspaces.map(p => <option key={p}>{p}</option>)}</select> : <code title={path}>{path || "Finding local checkout…"}</code>}
      {context && !context.checkout_exists && <small>A working checkout will be created when you open it.</small>}
    </div></div>
    <details className="open-dropdown" ref={dropdown}>
      <summary aria-label="Open repository"><Terminal size={15} />{busy ? "Opening…" : "Open"}<ChevronDown size={14} /></summary>
      <div className="open-menu">
        <small>Available on this computer</small>
        {context?.tools.map(tool => <button key={tool.id} disabled={busy || !tool.available} title={tool.reason} onClick={() => launch(tool.id)}><span>{tool.label}</span>{!tool.available && <small>{tool.reason}</small>}</button>)}
        <button disabled={busy} onClick={() => setRefresh(n => n + 1)}><RefreshCw size={13} /> Refresh detection</button>
      </div>
    </details>
    {message && <small className="launch-message" role="status">{message}</small>}
  </div>;
}

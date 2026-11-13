export type Repo = {
  full_name: string;
  description: string | null;
  default_branch: string;
  last_sync: string | null;
  issue_count: number;
  pr_count: number;
  path: string;
};
export type Item = {
  number: number;
  title: string;
  body: string | null;
  state: string;
  user?: { login: string };
  labels?: { name: string; color: string }[];
  comments: number;
  created_at: string;
  updated_at: string;
  pull_request?: unknown;
  merged_at?: string;
  draft?: boolean;
  head?: { ref: string };
  base?: { ref: string };
};
export type Comment = {
  id: number;
  body: string;
  user?: { login: string };
  created_at?: string;
  submitted_at?: string;
  state?: string;
  path?: string;
  line?: number;
};
export type Detail = {
  item: Item;
  comments: Comment[];
  reviews: Comment[];
  review_comments: Comment[];
  files: {
    filename: string;
    status: string;
    additions: number;
    deletions: number;
    patch?: string;
  }[];
  diff: string;
};
export type Entry = { name: string; path: string; kind: "tree" | "blob" };
export async function api<T>(url: string, init?: RequestInit): Promise<T> {
  const r = await fetch(url, {
    ...init,
    headers: { "Content-Type": "application/json", ...init?.headers },
  });
  if (!r.ok) {
    const data = await r.json().catch(() => null);
    throw new Error(data?.error || `Request failed (${r.status})`);
  }
  return r.json();
}
export const repoUrl = (repo: string) =>
  "/api/repos/" + repo.split("/").map(encodeURIComponent).join("/");

export type Route = { repo: string; page: string; number: number | null };
export function readRoute(): Route {
  const match = /^\/([\w.-]+)\/([\w.-]+)(?:\/(issues|pull|pulls|tree|outgoing)(?:\/(\d+))?)?\/?$/.exec(window.location.pathname);
  if (!match) return { repo: "", page: "issue", number: null };
  const page = match[3] === "pull" || match[3] === "pulls" ? "pr" : match[3] === "tree" ? "code" : match[3] === "outgoing" ? "outgoing" : "issue";
  const number = match[4] ? Number(match[4]) : null;
  return { repo: `${match[1]}/${match[2]}`, page, number: (page === "issue" || page === "pr") && Number.isSafeInteger(number) && number! > 0 ? number : null };
}
export function routePath(route: Route): string {
  if (!route.repo) return "/";
  const section = route.page === "pr" ? route.number === null ? "pulls" : "pull" : route.page === "code" ? "tree" : route.page === "outgoing" ? "outgoing" : "issues";
  return `/${route.repo}/${section}${route.number === null ? "" : `/${route.number}`}`;
}
export function numberQuery(query: string): boolean {
  return /^(?:#|number:)?[1-9]\d*$/.test(query.trim());
}

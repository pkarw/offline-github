import type { Item, Detail } from './api';
import type { Event } from './outbox';

export const itemTarget = (item: Item) => item.local_target || String(item.number);
export const itemNumber = (item: Item) => typeof item.number === 'string' ? 'Local draft' : `#${item.number}`;
export function itemEvents(item: Item, events: Event[]) {
  const creation = events.find(e => ['issue_create', 'pr_create'].includes(e.kind) && e.remote?.number === item.number);
  const alias = item.local_target || (creation ? `local:${creation.id}` : undefined);
  return events.filter(e => e.target === String(item.number) || (alias && (e.target === alias || e.id === alias.slice(6))));
}
// Project the durable journal without changing the incoming GitHub snapshot.
export function localItems(events: Event[], cached: Item[], lastSync?: string | null): Item[] {
  return events.filter(e => ['issue_create', 'pr_create'].includes(e.kind) && e.status !== 'cancelled')
    .filter(e => !e.remote?.number || !cached.some(i => i.number === e.remote?.number))
    // A completed Pull owns published items, including ones excluded by current filters.
    .filter(e => !(e.status === 'applied' && lastSync && e.history?.some(h => h.action === 'github_confirmed' && Date.parse(lastSync) > Date.parse(h.at))))
    .map(e => {
      const target = `local:${e.id}`;
      const item: Item = {
        number: e.remote?.number || target, local_target: target, title: String(e.payload.title),
        body: String(e.payload.body || ''), state: 'open', user: {login: 'You (local)'},
        labels: Array.isArray(e.payload.labels) ? e.payload.labels.map(name => ({name: String(name), color: '808080'})) : [],
        comments: 0, created_at: e.created_at, updated_at: e.created_at,
        ...(e.kind === 'pr_create' ? {pull_request: {}, draft: Boolean(e.payload.draft), head: {ref: String(e.payload.head)}, base: {ref: String(e.payload.base)}} : {}),
      };
      for (const change of events.filter(v => v.target === target && v.status !== 'cancelled').sort((a,b) => a.seq - b.seq)) {
        if (change.kind === 'edit' && change.payload.fields && typeof change.payload.fields === 'object') {
          const fields = change.payload.fields as Record<string, unknown>;
          if (typeof fields.title === 'string') item.title = fields.title;
          if (typeof fields.body === 'string') item.body = fields.body;
        }
        if (change.kind === 'labels') {
          const remove = Array.isArray(change.payload.remove) ? change.payload.remove : [];
          item.labels = item.labels?.filter(l => !remove.includes(l.name));
          for (const name of Array.isArray(change.payload.add) ? change.payload.add : [])
            if (!item.labels?.some(l => l.name === name)) item.labels?.push({name: String(name), color: '808080'});
        }
        if (change.kind === 'comment') item.comments++;
      }
      return item;
    });
}
export function localDetail(item: Item): Detail {
  return {item, comments: [], reviews: [], review_comments: [], files: [], diff: ''};
}

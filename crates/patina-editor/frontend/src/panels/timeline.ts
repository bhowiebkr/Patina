import type { Domain, NodeTime } from '../api';

/** The time of one audio block: 256 samples at 48 kHz (patina_sdk::audio, 15.7). */
export const BLOCK_MS = (256 * 1000) / 48000;
const ORDER: readonly Domain[] = ['frame', 'audio', 'event', 'batch'];

/** The calls of one thread, sorted by start. */
export interface Row {
  thread: number;
  calls: NodeTime[];
}

/** The calls of one domain. `limit` is the frame budget or the block deadline in milliseconds. */
export interface Lane {
  domain: Domain;
  limit: number | null;
  span: number;
  rows: Row[];
}

/** Groups the node times by domain and by thread for the timeline of the **Debug** tab (15.7). */
export function lanes(times: Iterable<NodeTime>, domains: Readonly<Record<string, Domain>>, frameRate: number): Lane[] {
  const by = new Map<Domain, Map<number, NodeTime[]>>();
  for (const t of times) {
    const domain = domains[t.id] ?? 'batch';
    let threads = by.get(domain);
    if (!threads) by.set(domain, (threads = new Map()));
    let calls = threads.get(t.thread);
    if (!calls) threads.set(t.thread, (calls = []));
    calls.push(t);
  }
  return ORDER.flatMap((domain) => {
    const threads = by.get(domain);
    if (!threads) return [];
    const rows = [...threads].sort(([a], [b]) => a - b).map(([thread, calls]) => ({ thread, calls: calls.sort((a, b) => a.start - b.start) }));
    const span = rows.reduce((max, r) => r.calls.reduce((m, c) => Math.max(m, c.start + c.ms), max), 0);
    const limit = domain === 'frame' && frameRate > 0 ? 1000 / frameRate : domain === 'audio' ? BLOCK_MS : null;
    return [{ domain, limit, span, rows }];
  });
}

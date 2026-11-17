// The test graph and the frame statistics of the canvas bench (`bench.ts`).

import type { Edit } from '../api';

/** The number, the mean, the 95th percentile and the largest of the frame times, in ms. */
export function frameStats(times: number[]): { n: number; mean: number; p95: number; max: number } {
  const sorted = [...times].sort((a, b) => a - b);
  const round = (ms: number) => Math.round(ms * 10) / 10;
  const mean = sorted.reduce((sum, t) => sum + t, 0) / Math.max(sorted.length, 1);
  return { n: times.length, mean: round(mean), p95: round(sorted[Math.floor(sorted.length * 0.95)] ?? 0), max: round(sorted.at(-1) ?? 0) };
}

const hexId = (prefix: string, i: number) => prefix + i.toString(16).padStart(15, '0');

/** One edit that adds `n` nodes in a grid: pairs of a number node and a show node with a wire. */
export function benchGraph(n: number): Edit {
  const edits: Edit[] = [];
  const columns = Math.ceil(Math.sqrt(n / 2));
  for (let i = 0; i < n / 2; i++) {
    const x = (i % columns) * 520;
    const y = Math.floor(i / columns) * 160;
    const [a, b] = [hexId('a', i), hexId('b', i)];
    edits.push({ kind: 'AddNode', id: a, folder: 'nodes/number', pos: [x, y] });
    edits.push({ kind: 'AddNode', id: b, folder: 'nodes/show', pos: [x + 260, y] });
    edits.push({ kind: 'Connect', from: `${a}.out`, to: `${b}.value` });
  }
  return { kind: 'Group', edits };
}

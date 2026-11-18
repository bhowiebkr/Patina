// The path bar of an open subgraph (spec 15.9): one crumb for each use from the main graph.

import { folderName, type NodeEntry } from '../api.ts';
import type { RunState } from './NodeView.tsx';

/** A graph in the path. `use` is the ID of its use in the graph before it, or '' for no use. */
export type Crumb = { graph: string; use: string; label: string };

/** The name of a use: the subgraph name, with a number when the graph has more uses of it. */
export function crumbLabel(nodes: NodeEntry[], use: string): string {
  const entry = nodes.find((n) => n.id === use);
  if (!entry) return use;
  const same = nodes.filter((n) => n.subgraph !== undefined && n.subgraph === entry.subgraph);
  return same.length > 1 ? `${folderName(entry)} ${same.indexOf(entry) + 1}` : folderName(entry);
}

/** The path to the open graph. A graph that the path does not end at starts a path with no use. */
export function trailOf(trail: Crumb[], graph: string): Crumb[] {
  if (graph === 'main') return [];
  return trail.at(-1)?.graph === graph ? trail : [{ graph, use: '', label: graph }];
}

/** The use path of the run IDs (6.4). A path with a graph with no use gives the only use of the
 * open graph in `uses`, or '' when the graph has no use or more than one. */
/** The engine ID of node `id` of a graph open at the use path `scope` (6.4). */
export function engineId(scope: string, id: string): string {
  return scope ? `${scope}/${id}` : id;
}

/**
 * The node GUIs by "<mode> <id>", with the IDs inside the use `scope` (6.4). `scope` null is the
 * main graph. A subgraph with no use (`scope` '') has no GUIs.
 */
export function scopeGuis(guis: ReadonlyMap<string, string>, scope: string | null): ReadonlyMap<string, string> {
  if (scope === null) return guis;
  const inner = new Map<string, string>();
  for (const [key, tree] of guis) {
    const at = key.indexOf(' ');
    const id = key.slice(at + 1);
    if (scope && id.startsWith(`${scope}/`)) inner.set(`${key.slice(0, at)} ${id.slice(scope.length + 1)}`, tree);
  }
  return inner;
}

/**
 * The values, idle nodes, times, reports and logs of one use of a subgraph, with the IDs inside
 * the use (6.4, 15.9).
 */
export function scopeRun(run: RunState, scope: string): RunState {
  if (!scope) return run;
  const p = `${scope}/`;
  const inner = <T,>(map: ReadonlyMap<string, T>) => [...map].filter(([k]) => k.startsWith(p)).map(([k, v]) => [k.slice(p.length), v] as const);
  const own = <T extends { id: string }>(list: T[]) => list.flatMap((x) => (x.id.startsWith(p) ? [{ ...x, id: x.id.slice(p.length) }] : []));
  const values = new Map(inner(run.values).map(([k, v]) => [k, { ...v, from: k }]));
  // The outputs of the use go to the public output pins, as "out:<key>.pin".
  for (const [k, v] of run.values) {
    if (k.startsWith(`${scope}.`)) values.set(`out:${k.slice(scope.length + 1)}.pin`, { ...v, from: `out:${k.slice(scope.length + 1)}.pin` });
  }
  return { ...run, values, idle: new Map(inner(run.idle)), times: new Map(inner(run.times)), reports: own(run.reports), logs: own(run.logs) };
}

export function usePathOf(trail: Crumb[], uses: readonly string[]): string {
  if (trail.every((c) => c.use)) return trail.map((c) => c.use).join('/');
  return uses.length === 1 ? uses[0] : '';
}

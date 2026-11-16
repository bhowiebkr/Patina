// The path bar of an open subgraph (spec 15.9): one crumb for each use from the main graph.

import { folderName, type NodeEntry } from './api.ts';

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

/** The use path of the run IDs (6.4), or '' when the path has a graph with no use. */
export function usePathOf(trail: Crumb[]): string {
  return trail.every((c) => c.use) ? trail.map((c) => c.use).join('/') : '';
}

// The lost pins of a node: wire ends at pin keys that the node does not have (15.9). A removed
// public pin of a subgraph or a renamed parameter makes them. Each gets a row and a handle, so its
// wire shows and the user can delete it.

import type { GraphFile } from '../api';

/** The input and output keys of the wire ends that a node has no pin for. */
export interface Lost {
  inputs: string[];
  outputs: string[];
}

interface Pins {
  inputs: readonly { key: string }[];
  outputs: readonly { key: string }[];
}

/** The pin keys that the wires and public pins end at, for each node ID: `i:<key>` and `o:<key>`. */
export function wireEnds(graph: GraphFile): Map<string, Set<string>> {
  const ends = new Map<string, Set<string>>();
  const add = (end: string, side: 'i' | 'o') => {
    const dot = end.indexOf('.');
    if (dot < 0) return;
    const id = end.slice(0, dot);
    let keys = ends.get(id);
    if (!keys) ends.set(id, (keys = new Set()));
    keys.add(`${side}:${end.slice(dot + 1)}`);
  };
  for (const w of graph.wire ?? []) {
    add(w.from, 'o');
    add(w.to, 'i');
  }
  for (const p of graph.input ?? []) for (const to of p.to ?? []) add(to, 'i');
  for (const p of graph.output ?? []) if (p.from) add(p.from, 'o');
  return ends;
}

/** The ends of `ends` that `pins` has no pin for. A node with no pins, for example a missing folder, loses all ends. */
export function lostPins(pins: Pins | null, ends: ReadonlySet<string> | undefined): Lost | undefined {
  if (!ends) return undefined;
  const lost: Lost = { inputs: [], outputs: [] };
  for (const end of ends) {
    const [input, key] = [end.startsWith('i:'), end.slice(2)];
    if (!(input ? pins?.inputs : pins?.outputs)?.some((p) => p.key === key)) (input ? lost.inputs : lost.outputs).push(key);
  }
  return lost.inputs.length + lost.outputs.length > 0 ? lost : undefined;
}

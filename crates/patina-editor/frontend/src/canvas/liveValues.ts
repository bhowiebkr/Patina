// The live values of the canvas: the stale rule and the values of the wires (15.9).

import type { Edge } from '@xyflow/react';
import type { RunState } from './NodeView';
import type { WireData } from '../wires/WireLayer';

/** True when the value of output `from`, "<node id>.<output key>", is from an old run. */
export function valueStale(run: RunState, from: string): boolean {
  return run.stale || run.idle.has(from.slice(0, from.lastIndexOf('.')));
}

/**
 * The edges with the live value of each wire, so a run frame renders only the wires whose value
 * changed (15.9). Gives `edges` itself when no value changed.
 */
export function withValues(edges: Edge<WireData>[], run: RunState): Edge<WireData>[] {
  let changed = false;
  const next = edges.map((e) => {
    if (!e.data) return e;
    const [value, old] = [run.values.get(e.data.from), valueStale(run, e.data.from)];
    if (e.data.value === value && e.data.old === old) return e;
    changed = true;
    return { ...e, data: { ...e.data, value, old } };
  });
  return changed ? next : edges;
}

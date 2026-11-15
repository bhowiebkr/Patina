// The rows of the Layers panel (spec 7.3, 15.2): the compilation base and the slot, each with its
// build state and the time of its last build that compiled. These are pure functions for `npm test`.

import type { RunState } from './NodeView';

export interface LayerRow {
  name: 'Base' | 'Slot';
  state: string;
  failed: boolean;
  /** The time of the last build of this layer that compiled, in milliseconds. */
  ms: number | null;
}

type Layers = Pick<RunState, 'phase' | 'errors' | 'fused' | 'slot' | 'baseMs' | 'slotMs'>;

/** A build or an error goes to the slot row when a node is in the slot, else to the base row. */
export function layerRows(run: Layers): LayerRow[] {
  const failed = run.phase === 'failed' || run.errors.length > 0;
  const own = (layer: boolean, done: string) => (layer && run.phase === 'building' ? 'Building' : layer && failed ? 'Build failed' : done);
  const inSlot = run.slot !== null;
  const base = run.phase === 'none' ? 'Not built' : own(!inSlot, run.fused ? 'Fused' : 'Edit build');
  return [
    { name: 'Base', state: base, failed: !inSlot && failed, ms: run.baseMs },
    { name: 'Slot', state: inSlot ? `${run.slot} - ${own(true, 'Edit build')}` : 'Empty', failed: inSlot && failed, ms: inSlot ? run.slotMs : null },
  ];
}

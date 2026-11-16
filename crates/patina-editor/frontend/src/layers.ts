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

type Layers = Pick<RunState, 'phase' | 'errors' | 'fused' | 'slot' | 'baseMs' | 'slotMs' | 'building'>;

/** A build shows on the rows of the layers that it builds. An error goes to the slot row when a
 * node is in the slot, else to the base row. */
export function layerRows(run: Layers): LayerRow[] {
  const failed = run.phase === 'failed' || run.errors.length > 0;
  const building = run.phase === 'building';
  const inSlot = run.slot !== null;
  const done = (layer: boolean, state: string) => (layer && failed ? 'Build failed' : state);
  const base = building && run.building.base ? 'Building' : run.phase === 'none' ? 'Not built' : done(!inSlot, run.fused ? 'Fused' : 'Edit build');
  const slot = building && run.building.slot !== null ? `${run.building.slot} - Building` : inSlot ? `${run.slot} - ${done(true, 'Edit build')}` : 'Empty';
  return [
    { name: 'Base', state: base, failed: !inSlot && failed, ms: run.baseMs },
    { name: 'Slot', state: slot, failed: inSlot && failed, ms: inSlot ? run.slotMs : null },
  ];
}

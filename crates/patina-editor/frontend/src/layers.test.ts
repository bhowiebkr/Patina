// The tests of the Layers panel rows (spec 7.3).

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { layerRows } from './layers.ts';

type Layers = Parameters<typeof layerRows>[0];
const run: Layers = { phase: 'ran', errors: [], fused: false, slot: null, baseMs: 1800, slotMs: null, building: { base: true, slot: null } };
const states = (r: Layers) => layerRows(r).map((row) => [row.state, row.failed, row.ms]);

test('the base row shows the build mode, and an empty slot has no time', () => {
  assert.deepEqual(states(run), [['Edit build', false, 1800], ['Empty', false, null]]);
  assert.deepEqual(states({ ...run, fused: true, slotMs: 400 }), [['Fused', false, 1800], ['Empty', false, null]]);
  assert.deepEqual(states({ ...run, phase: 'none', baseMs: null }), [['Not built', false, null], ['Empty', false, null]]);
});

test('a build shows on the rows of the layers that it builds', () => {
  assert.deepEqual(states({ ...run, phase: 'building' }), [['Building', false, 1800], ['Empty', false, null]]);
  const slotted: Layers = { ...run, slot: 'number', slotMs: 400 };
  const slotOnly = { base: false, slot: 'number' };
  assert.deepEqual(states({ ...slotted, phase: 'building', building: slotOnly }), [['Edit build', false, 1800], ['number - Building', false, 400]]);
  // A first save of a node folder builds the base with the new slot (7.1).
  assert.deepEqual(states({ ...run, phase: 'building', building: { base: true, slot: 'mix' } }), [['Building', false, 1800], ['mix - Building', false, null]]);
  // A fuse builds the base with no slot.
  assert.deepEqual(states({ ...slotted, phase: 'building' }), [['Building', false, 1800], ['number - Edit build', false, 400]]);
});

test('the errors show on the slot row when a node is in the slot', () => {
  const slotted: Layers = { ...run, slot: 'number', slotMs: 400 };
  assert.deepEqual(states(slotted), [['Edit build', false, 1800], ['number - Edit build', false, 400]]);
  const errors = [{ rendered: 'x' }] as unknown as Layers['errors'];
  assert.deepEqual(states({ ...slotted, errors }), [['Edit build', false, 1800], ['number - Build failed', true, 400]]);
  assert.deepEqual(states({ ...run, phase: 'failed' }), [['Build failed', true, 1800], ['Empty', false, null]]);
});

test('the end of the node process keeps the build state', () => {
  assert.deepEqual(states({ ...run, phase: 'lost' }), [['Edit build', false, 1800], ['Empty', false, null]]);
});

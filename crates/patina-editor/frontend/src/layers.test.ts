// The tests of the Layers panel rows (spec 7.3).

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { layerRows } from './layers.ts';

type Layers = Parameters<typeof layerRows>[0];
const run: Layers = { phase: 'ran', errors: [], fused: false, slot: null, baseMs: 1800, slotMs: null };
const states = (r: Layers) => layerRows(r).map((row) => [row.state, row.failed, row.ms]);

test('the base row shows the build mode, and an empty slot has no time', () => {
  assert.deepEqual(states(run), [['Edit build', false, 1800], ['Empty', false, null]]);
  assert.deepEqual(states({ ...run, fused: true, slotMs: 400 }), [['Fused', false, 1800], ['Empty', false, null]]);
  assert.deepEqual(states({ ...run, phase: 'none', baseMs: null }), [['Not built', false, null], ['Empty', false, null]]);
});

test('a build and its errors show on the slot row when a node is in the slot', () => {
  const slotted: Layers = { ...run, slot: 'number', slotMs: 400 };
  assert.deepEqual(states(slotted), [['Edit build', false, 1800], ['number - Edit build', false, 400]]);
  assert.deepEqual(states({ ...slotted, phase: 'building' }), [['Edit build', false, 1800], ['number - Building', false, 400]]);
  const errors = [{ rendered: 'x' }] as unknown as Layers['errors'];
  assert.deepEqual(states({ ...slotted, errors }), [['Edit build', false, 1800], ['number - Build failed', true, 400]]);
  assert.deepEqual(states({ ...run, phase: 'failed' }), [['Build failed', true, 1800], ['Empty', false, null]]);
});

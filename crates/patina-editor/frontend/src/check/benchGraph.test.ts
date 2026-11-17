import { test } from 'node:test';
import assert from 'node:assert/strict';
import { benchGraph, frameStats } from './benchGraph.ts';

test('the frame statistics', () => {
  const times = Array.from({ length: 20 }, (_, i) => i + 1);
  assert.deepEqual(frameStats(times), { n: 20, mean: 10.5, p95: 20, max: 20 });
  assert.deepEqual(frameStats([]), { n: 0, mean: 0, p95: 0, max: 0 });
});

test('the bench graph has n nodes with one wire for each pair', () => {
  const g = benchGraph(8);
  assert.equal(g.kind, 'Group');
  if (g.kind !== 'Group') return;
  const adds = g.edits.filter((e) => e.kind === 'AddNode');
  assert.equal(adds.length, 8);
  assert.equal(new Set(adds.map((e) => (e.kind === 'AddNode' ? e.id : ''))).size, 8);
  assert.ok(adds.every((e) => e.kind === 'AddNode' && /^[0-9a-f]{16}$/.test(e.id)));
  assert.equal(g.edits.filter((e) => e.kind === 'Connect').length, 4);
});

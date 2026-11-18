// The tests of the live wire values. `npm test` runs them.

import assert from 'node:assert/strict';
import { test } from 'node:test';
import type { Edge } from '@xyflow/react';
import type { Value } from '../api.ts';
import type { RunState } from './NodeView.tsx';
import type { WireData } from '../wires/WireLayer.tsx';
import { withValues } from './liveValues.ts';

const runOf = (values: Value[], idle: string[] = []) =>
  ({ stale: false, idle: new Map(idle.map((id) => [id, true])), values: new Map(values.map((v) => [v.from, v])) }) as unknown as RunState;
const wire = (from: string): Edge<WireData> => ({ id: `${from}>b.in`, source: 'a', target: 'b', data: { from, to: 'b.in', route: [] } });

test('withValues changes only the wires whose value changed', () => {
  const [one, two] = [{ from: 'a.x', text: '1' }, { from: 'a.y', text: '2' }];
  const edges = withValues([wire('a.x'), wire('a.y'), { id: 'order:a>b', source: 'a', target: 'b' }], runOf([one, two]));
  assert.equal(edges[0].data?.value, one);
  assert.equal(edges[1].data?.old, false);
  // The same values give the same array, so React Flow renders nothing.
  assert.equal(withValues(edges, runOf([one, two])), edges);
  const next = withValues(edges, runOf([{ from: 'a.x', text: '3' }, two]));
  assert.notEqual(next[0], edges[0]);
  assert.equal(next[0].data?.route, edges[0].data?.route);
  assert.equal(next[1], edges[1]);
  assert.equal(next[2], edges[2]);
  // An idle node makes its values old.
  assert.equal(withValues(next, runOf([{ from: 'a.x', text: '3' }, two], ['a']))[1].data?.old, true);
});

// The tests of the path bar of an open subgraph. `npm test` runs them.

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { crumbLabel, engineId, scopeGuis, scopeRun, trailOf, usePathOf, type Crumb } from './crumbs.ts';
import type { RunState } from './NodeView.tsx';

test('a use gets a number only when the graph has more uses of the subgraph', () => {
  const nodes = [
    { id: 'a', subgraph: 'half-adder' },
    { id: 'b', folder: 'nodes/or' },
    { id: 'c', subgraph: 'graphs/half-adder' },
    { id: 'd', subgraph: 'half-adder' },
  ];
  assert.equal(crumbLabel(nodes, 'a'), 'half-adder 1');
  assert.equal(crumbLabel(nodes, 'd'), 'half-adder 2');
  assert.equal(crumbLabel(nodes, 'c'), 'half-adder');
  assert.equal(crumbLabel(nodes, 'x'), 'x');
});

test('the path ends at the open graph, or starts again with no use', () => {
  const trail: Crumb[] = [
    { graph: 'alu', use: 'u1', label: 'alu' },
    { graph: 'adder', use: 'u2', label: 'adder 2' },
  ];
  assert.deepEqual(trailOf(trail, 'adder'), trail);
  assert.deepEqual(trailOf(trail, 'main'), []);
  const loose = trailOf(trail, 'bank');
  assert.deepEqual(loose, [{ graph: 'bank', use: '', label: 'bank' }]);
  assert.equal(usePathOf(trail, ['u7']), 'u1/u2');
  assert.equal(usePathOf([...loose, { graph: 'cell', use: 'u3', label: 'cell' }], []), '');
  assert.equal(usePathOf([], []), '');
  assert.equal(usePathOf(loose, ['u1/u4']), 'u1/u4', 'the only use gives the live values');
  assert.equal(usePathOf(loose, ['u1/u4', 'u5']), '', 'two uses give no use');
});

test('the GUIs of an open subgraph use have the IDs inside the use, and a subgraph with no use has none', () => {
  const guis = new Map([['0 a', 'x'], ['0 u/b', 'y'], ['1 u/b', 'z'], ['0 u/v/c', 'w'], ['0 uu/d', 'q']]);
  assert.equal(scopeGuis(guis, null), guis);
  assert.deepEqual([...scopeGuis(guis, 'u')], [['0 b', 'y'], ['1 b', 'z'], ['0 v/c', 'w']]);
  assert.equal(scopeGuis(guis, '').size, 0);
  assert.equal(engineId('u', 'b'), 'u/b');
  assert.equal(engineId('', 'b'), 'b');
});

test('the run of an open use has the reports, logs and times of its inner nodes only', () => {
  const report = (id: string) => ({ id, folder: 'nodes/x', kind: 'Error' as const, text: 'bad' });
  const log = (id: string) => ({ id, folder: 'nodes/x', level: 'INFO', text: 'hi' });
  const time = { ms: 1, startMs: 0, thread: 0 };
  const run = { values: new Map(), idle: new Map(), times: new Map([['u/a', time], ['b', time]]), reports: [report('u/a'), report('b')], logs: [log('u/v/c'), log('uu/d')] } as unknown as RunState;
  assert.equal(scopeRun(run, ''), run);
  const inner = scopeRun(run, 'u');
  assert.deepEqual([...inner.times.keys()], ['a']);
  assert.deepEqual(inner.reports.map((r) => r.id), ['a']);
  assert.deepEqual(inner.logs.map((l) => l.id), ['v/c']);
});

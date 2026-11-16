// The tests of the path bar of an open subgraph. `npm test` runs them.

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { crumbLabel, trailOf, usePathOf, type Crumb } from './crumbs.ts';

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
  assert.equal(usePathOf(trail), 'u1/u2');
  assert.equal(usePathOf([...loose, { graph: 'cell', use: 'u3', label: 'cell' }]), '');
  assert.equal(usePathOf([]), '');
});

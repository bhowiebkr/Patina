import assert from 'node:assert/strict';
import { test } from 'node:test';
import { NODE_W, network, nodeHeight } from './backdrop.ts';

test('a larger screen shows more nodes of the same size, and the nodes do not overlap', () => {
  const small = network(800, 600);
  const big = network(3840, 2160);
  assert.ok(big.nodes.length > 3 * small.nodes.length);
  const keys = new Set(big.nodes.map((n) => JSON.stringify(n)));
  for (const n of small.nodes) assert.ok(keys.has(JSON.stringify(n)), 'a cell keeps its node');
  assert.ok(big.nodes.some((n) => n.x + NODE_W >= 3840) && big.nodes.some((n) => n.y + nodeHeight(n) >= 2160));
  for (const [i, a] of small.nodes.entries()) {
    for (const b of small.nodes.slice(i + 1)) {
      const apart = a.x + NODE_W <= b.x || b.x + NODE_W <= a.x || a.y + nodeHeight(a) <= b.y || b.y + nodeHeight(b) <= a.y;
      assert.ok(apart, `${JSON.stringify(a)} overlaps ${JSON.stringify(b)}`);
    }
  }
  for (const [a, o, b, i] of big.wires) assert.ok(o < big.nodes[a].outs && i < big.nodes[b].ins);
});

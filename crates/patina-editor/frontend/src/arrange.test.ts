import { test } from 'node:test';
import assert from 'node:assert/strict';
import { arrange, type Box, type Link } from './arrange.ts';

const box = (id: string, side?: Box['side']): Box => ({ id, w: 100, h: 60, side });
const link = (from: string, to: string): Link => ({ from, fy: 30, to, ty: 30, net: from });

test('the signal flows from left to right', () => {
  const at = arrange([box('c'), box('b'), box('a')], [link('a', 'b'), link('b', 'c')]);
  assert.ok(at.get('a')![0] < at.get('b')![0] && at.get('b')![0] < at.get('c')![0]);
});

test('the nodes of a column do not overlap', () => {
  const boxes = [box('s'), box('x'), box('y'), box('z')];
  const at = arrange(boxes, [link('s', 'x'), link('s', 'y'), link('s', 'z')]);
  const ys = ['x', 'y', 'z'].map((id) => at.get(id)!).sort((p, q) => p[1] - q[1]);
  assert.ok(ys.every((p) => p[0] === ys[0][0]));
  for (let k = 1; k < ys.length; k++) assert.ok(ys[k][1] - ys[k - 1][1] >= 60 + 32);
});

test('a loop and the public pins get a place', () => {
  const boxes = [box('in', 'first'), box('a'), box('b'), box('out', 'last'), box('free')];
  const at = arrange(boxes, [link('in', 'a'), link('a', 'b'), link('b', 'a'), link('b', 'out')]);
  const x = (id: string) => at.get(id)![0];
  assert.equal(at.size, 5);
  assert.ok(x('in') < x('a') && x('a') < x('b') && x('b') < x('out'));
  assert.ok(boxes.every((b) => x(b.id) <= x('out') && x(b.id) >= x('in')));
});

test('a gap gets a lane only for a wire with a bend', () => {
  const straight = arrange([box('a'), box('b')], [link('a', 'b')]);
  assert.equal(straight.get('b')![0] - straight.get('a')![0], 100 + 80);
  const bent = arrange([box('a'), box('b'), box('c')], [link('a', 'b'), link('a', 'c')]);
  assert.ok(bent.get('b')![0] - bent.get('a')![0] > 100 + 80);
});

test('bends at different heights share a lane', () => {
  // Two outputs bend in the same gap, one above the other, so the gap needs one lane.
  const ids = ['s1', 't1', 'u1', 's2', 't2', 'u2'];
  const at = arrange(ids.map((id) => box(id)), [link('s1', 't1'), link('s1', 'u1'), link('s2', 't2'), link('s2', 'u2')]);
  assert.equal(at.get('t1')![0] - at.get('s1')![0], 100 + 80 + 12);
});

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

test('a bus gets a lane as wide as the bus', () => {
  const gap = (width: number) => {
    const at = arrange([box('a'), box('b'), box('c')], [{ ...link('a', 'b'), width }, { ...link('a', 'c'), width }]);
    return at.get('b')![0] - at.get('a')![0];
  };
  assert.equal(gap(2), 100 + 80 + 12);
  assert.equal(gap(40), 100 + 80 + 40 + 8);
});

test('a bus stays straight before a thin wire', () => {
  // The node t gets a bus from p and a thin wire from q, so t moves to the height of p.
  const at = arrange([box('p'), box('q'), box('t')], [{ ...link('p', 't'), width: 40 }, link('q', 't')]);
  assert.equal(at.get('t')![1], at.get('p')![1]);
  assert.notEqual(at.get('t')![1], at.get('q')![1]);
});

test('a long bus gets room for its width in each column that it goes through', () => {
  // The bus from a to c goes through the column of b and m. Each node keeps space from the bus.
  const boxes = [box('a'), box('b'), box('m'), box('c')];
  const at = arrange(boxes, [link('a', 'b'), link('b', 'c'), { ...link('a', 'c'), width: 40 }, link('a', 'm'), link('m', 'c')]);
  // The bus stays at the height of its output (1 px or less), 28 px plus 16 px from each node.
  const bus = at.get('a')![1] + 30;
  for (const id of ['b', 'm']) {
    const y = at.get(id)![1];
    assert.ok(bus <= y - 28 - 16 + 1 || bus >= y + 60 + 28 + 16 - 1, `${id} ${y} ${bus}`);
  }
});

test('a tall column splits into columns in turn', () => {
  // 40 sources of one reader make a column that is much taller than the layout is wide.
  const ids = Array.from({ length: 40 }, (_, k) => `s${k}`);
  const boxes = [...ids.map((id) => box(id)), box('r')];
  const at = arrange(boxes, ids.map((id) => link(id, 'r')));
  const pos = boxes.map((b) => at.get(b.id)!);
  const xs = [...new Set(ids.map((id) => at.get(id)![0]))].sort((p, q) => p - q);
  assert.ok(xs.length > 1 && xs.length <= 4, `${xs}`);
  assert.ok(at.get('r')![0] > xs[xs.length - 1]);
  // The sources go to the columns in turn, so the columns get the same count, plus or minus 1.
  const counts = xs.map((x) => ids.filter((id) => at.get(id)![0] === x).length);
  assert.ok(Math.max(...counts) - Math.min(...counts) <= 1, `${counts}`);
  // One column is 40 x 92 px tall. The wires that go through the split columns take space too.
  assert.ok(Math.max(...pos.map((p) => p[1])) < 0.75 * 40 * 92);
  for (let i = 0; i < pos.length; i++)
    for (let j = i + 1; j < pos.length; j++) {
      const [a, b] = [pos[i], pos[j]];
      assert.ok(Math.abs(a[0] - b[0]) >= 100 || Math.abs(a[1] - b[1]) >= 60, `${boxes[i].id} ${boxes[j].id}`);
    }
});

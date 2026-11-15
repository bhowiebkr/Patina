// The tests of the wire geometry. `npm test` runs them with Node 22.18 or later, which strips the types.

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { HAS_LANES, insertAt, instances, LIT, MAX_POINTS, MAX_ROUTE, midpoint, normals, orthogonal, shape, STALE, STRIDE, WireStore, wirePath, wirePoints } from './wires.ts';
import type { Corners, Style } from './api.ts';

/** The miter normals of a line through the x, y pairs. */
function miters(...xy: number[]): number[][] {
  const n = xy.length / 2;
  const p = new Float32Array(n * STRIDE);
  for (let k = 0; k < n; k++) p.set([xy[k * 2], xy[k * 2 + 1]], k * STRIDE);
  normals(p, 0, n);
  return Array.from({ length: n }, (_, k) => [p[k * STRIDE + 2], p[k * STRIDE + 3]]);
}

function near(actual: number[][], expected: number[][]) {
  assert.equal(actual.length, expected.length);
  actual.forEach((v, k) => v.forEach((c, i) => assert.ok(Math.abs(c - expected[k][i]) < 1e-5, `point ${k}: ${v} is not ${expected[k]}`)));
}

test('a straight line has unit normals', () => {
  near(miters(0, 0, 10, 0, 20, 0), [[0, 1], [0, 1], [0, 1]]);
});

test('a right angle has a longer miter at the joint', () => {
  near(miters(0, 0, 10, 0, 10, 10), [[0, 1], [-1, 1], [-1, 0]]);
});

test('segments of zero length take the direction of a neighbor', () => {
  near(miters(0, 0, 0, 0, 10, 0, 10, 0), [[0, 1], [0, 1], [0, 1], [0, 1]]);
  near(miters(5, 5, 5, 5), [[0, 1], [0, 1]]);
});

test('a sharp bend keeps the miter at 4 times the half width or less', () => {
  for (const m of [...miters(0, 0, 10, 0, 0, 1), ...miters(0, 0, 10, 0, 0, 0)]) {
    assert.ok(Number.isFinite(m[0]) && Number.isFinite(m[1]));
    assert.ok(Math.hypot(m[0], m[1]) <= 4 + 1e-5);
  }
});

/** The x, y pairs of a path from (0, 0) to (tx, ty). */
function path(tx: number, ty: number, points: [number, number][], style?: Style, corners?: Corners): number[][] {
  const out = new Float32Array(MAX_POINTS * STRIDE);
  const n = wirePoints(0, 0, tx, ty, shape(0, 0, tx, ty, points, style, corners), out, 0);
  return Array.from({ length: n }, (_, k) => [out[k * STRIDE], out[k * STRIDE + 1]]);
}

/** The x, y pairs of a route from (0, 0) to (100, 0) with no style: a rounded polyline. */
function route(...points: [number, number][]): number[][] {
  return path(100, 0, points);
}

test('a route keeps its ends and rounds each corner with a radius of 12 px', () => {
  const p = route([50, 0], [50, 40]);
  assert.equal(p.length, 2 + 5 + 5);
  near([p[0], p[1], p[3], p[5], p[6], p[11]], [[0, 0], [38, 0], [47, 3], [50, 12], [50, 28], [100, 0]]);
  near([p[10]], [[50 + (50 / Math.hypot(50, 40)) * 12, 40 - (40 / Math.hypot(50, 40)) * 12]]);
});

test('a short segment makes a smaller corner', () => {
  for (const [x, y] of route([50, 0], [50, 4]).slice(0, -1)) assert.ok(x >= 0 && x <= 52 + 1e-5 && y >= -1e-5 && y <= 4 + 1e-5, `${x}, ${y}`);
});

test('the largest route fits in a row, and a longer route gets fewer points for each corner', () => {
  const zigzag = (n: number) => Array.from({ length: n }, (_, k): [number, number] => [k * 8, k % 2 ? 20 : 0]);
  assert.ok(route(...zigzag(MAX_ROUTE)).length <= MAX_POINTS);
  assert.ok(path(100, 0, zigzag(MAX_ROUTE), 'orthogonal').length <= MAX_POINTS);
  assert.ok(path(100, 0, zigzag(MAX_ROUTE), 'curved').length <= MAX_POINTS);
  assert.equal(route(...zigzag(40)).length, 42, 'sharp corners');
});

test('a new point goes into the nearest segment', () => {
  assert.equal(insertAt(0, 0, 100, 0, [], 50, 10), 0);
  const r: [number, number][] = [[50, 0], [50, 40]];
  assert.equal(insertAt(0, 0, 100, 0, r, 20, 2), 0);
  assert.equal(insertAt(0, 0, 100, 0, r, 52, 20), 1);
  assert.equal(insertAt(0, 0, 100, 0, r, 80, 15), 2);
});

test('the SVG path of a route has the same corners', () => {
  const r = shape(0, 0, 100, 0, [[50, 0], [50, 40]]);
  assert.ok(wirePath(0, 0, 100, 0, r).startsWith('M0 0L38 0Q50 0 50 12L50 28Q50 40 '));
  assert.ok(wirePath(0, 0, 100, 0, r).endsWith('L100 0'));
  assert.equal(wirePath(0, 0, 100, 0), 'M0 0C50 0 50 0 100 0');
});

test('a route with a new number of points rebuilds the segment lists', () => {
  const store = new WireStore();
  store.set('w', 0, 0, 100, 0, shape(0, 0, 100, 0, []), false);
  store.layout = false;
  store.set('w', 0, 0, 100, 0, shape(0, 0, 100, 0, [[50, 40]]), false);
  assert.ok(store.layout, 'the bezier and the route have different numbers of points');
  store.layout = false;
  store.set('w', 0, 0, 100, 0, shape(0, 0, 100, 0, [[50, 60]]), false);
  assert.ok(!store.layout, 'a moved point keeps the number of points');
});

test('a bus draws each lane of each segment, and a selected bus draws one ribbon', () => {
  const store = new WireStore();
  store.set('plain', 0, 0, 100, 0, shape(0, 0, 100, 0, []), false);
  store.set('bus', 0, 0, 100, 0, shape(0, 0, 100, 0, [[50, 40]]), true, '0110', true);
  const plain = store.counts[0] - 1;
  const bus = store.counts[1] - 1;
  const all = instances(store, false);
  assert.equal(all.length, (plain + bus * 4) * 4);
  assert.deepEqual([...all.subarray(0, 4)], [0, 0, 0, 0]);
  // Lane 0 is the most significant bit. Each instance keeps the lane count - 1.
  const flags = 3 | HAS_LANES | STALE;
  const first = [...all.subarray(plain * 4, plain * 4 + 16)];
  assert.deepEqual(first, [1, 0, 0, flags, 1, 0, 1, flags | LIT, 1, 0, 2, flags | LIT, 1, 0, 3, flags]);
  const chosen = instances(store, true);
  assert.equal(chosen.length, bus * 4);
  assert.deepEqual([...chosen.subarray(0, 4)], [1, 0, 0, flags]);
  store.layout = false;
  store.set('bus', 0, 0, 100, 0, shape(0, 0, 100, 0, [[50, 40]]), true, '0110', true);
  assert.ok(!store.layout, 'the same lanes keep the instances');
  store.set('bus', 0, 0, 100, 0, shape(0, 0, 100, 0, [[50, 40]]), true, '0111', true);
  assert.ok(store.layout, 'a new lane state needs new instances');
  for (let k = 0; k < 20; k++) store.set(`w${k}`, 0, 0, 10, 0, shape(0, 0, 10, 0, []), false, '1');
  assert.equal(store.bits.length, store.counts.length, 'the lanes grow with the slots');
  assert.equal(store.bits[21], '1');
});

test('the value badge sits at the middle of the curve or of the longest segment', () => {
  assert.deepEqual(midpoint(0, 0, 100, 40, shape(0, 0, 100, 40, [])), [50, 20]);
  // Segments of 10, 10 and 20: the badge is at the middle of the last one, not on a route point.
  assert.deepEqual(midpoint(0, 0, 30, 10, shape(0, 0, 30, 10, [[10, 0], [10, 10]])), [20, 10]);
  // Two equal segments: the first one wins, so the badge is not on the route point between them.
  assert.deepEqual(midpoint(0, 0, 20, 0, shape(0, 0, 20, 0, [[10, 0]])), [5, 0]);
  assert.deepEqual(midpoint(0, 0, 0, 0, shape(0, 0, 0, 0, [[0, 0]])), [0, 0]);
});

test('an orthogonal path has only horizontal and vertical segments, and is horizontal at the pins', () => {
  assert.deepEqual(orthogonal(0, 0, 100, 40, []), [[50, 0], [50, 40]]);
  assert.deepEqual(orthogonal(0, 0, 100, 0, []), [], 'pins in line need no bend');
  assert.deepEqual(orthogonal(100, 0, 0, 40, []), [[120, 0], [120, 20], [-20, 20], [-20, 40]], 'a path back leaves the pins first');
  const bends = orthogonal(0, 0, 100, 40, [[30, 10], [70, -20], [70, 5]]);
  assert.deepEqual(bends.slice(-2), [[85, 5], [85, 40]], 'the last leg turns at the middle');
  const all = [[0, 0], ...bends, [100, 40]];
  for (let k = 1; k < all.length; k++) assert.ok(all[k][0] === all[k - 1][0] || all[k][1] === all[k - 1][1], `segment ${k}`);
  assert.equal(all[1][1], 0);
  assert.equal(all[all.length - 2][1], 40);
  for (const p of [[30, 10], [70, -20], [70, 5]]) assert.ok(bends.some((b) => b[0] === p[0] && b[1] === p[1]), 'each route point is on the path');
});

test('sharp corners add no points, and rounded corners add points', () => {
  assert.equal(path(100, 40, [], 'orthogonal', 'sharp').length, 4);
  assert.equal(path(100, 40, [], 'orthogonal').length, 2 + 2 * 5);
  assert.equal(path(100, 40, [[50, 20]], 'polyline', 'sharp').length, 3);
  assert.equal(path(100, 40, [], 'polyline').length, 2, 'a polyline with no points is straight');
});

test('a curved path goes through its route points and keeps the default curve with none', () => {
  const p = path(100, 0, [[50, 40]], 'curved');
  assert.ok(p.length <= MAX_POINTS);
  near([p[0], p[p.length - 1]], [[0, 0], [100, 0]]);
  assert.ok(p.some(([x, y]) => Math.abs(x - 50) < 1e-4 && Math.abs(y - 40) < 1e-4), 'the curve passes the point');
  assert.ok(Math.abs(p[1][1]) < Math.abs(p[1][0]), 'the curve leaves the pin horizontally');
  assert.equal(path(100, 0, []).length, 33);
  near([midpoint(0, 0, 100, 0, shape(0, 0, 100, 0, [[50, 40]], 'curved'))], [[33.75, 20]]);
});

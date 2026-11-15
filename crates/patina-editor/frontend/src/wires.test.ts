// The tests of the wire geometry. `npm test` runs them with Node 22.18 or later, which strips the types.

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { insertAt, MAX_POINTS, MAX_ROUTE, midpoint, normals, routePoints, STRIDE, WireStore, wirePath } from './wires.ts';

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

/** The x, y pairs of a route from (0, 0) to (100, 0). */
function route(...points: [number, number][]): number[][] {
  const out = new Float32Array(MAX_POINTS * STRIDE);
  const n = routePoints(0, 0, 100, 0, points, out, 0);
  return Array.from({ length: n }, (_, k) => [out[k * STRIDE], out[k * STRIDE + 1]]);
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

test('the largest route fits in a row, and a longer route keeps sharp corners', () => {
  const zigzag = (n: number) => Array.from({ length: n }, (_, k): [number, number] => [k * 8, k % 2 ? 20 : 0]);
  assert.ok(route(...zigzag(MAX_ROUTE)).length <= MAX_POINTS);
  assert.equal(route(...zigzag(MAX_ROUTE + 1)).length, MAX_ROUTE + 3);
});

test('a new point goes into the nearest segment', () => {
  assert.equal(insertAt(0, 0, 100, 0, [], 50, 10), 0);
  const r: [number, number][] = [[50, 0], [50, 40]];
  assert.equal(insertAt(0, 0, 100, 0, r, 20, 2), 0);
  assert.equal(insertAt(0, 0, 100, 0, r, 52, 20), 1);
  assert.equal(insertAt(0, 0, 100, 0, r, 80, 15), 2);
});

test('the SVG path of a route has the same corners', () => {
  assert.ok(wirePath(0, 0, 100, 0, [[50, 0], [50, 40]]).startsWith('M0 0L38 0Q50 0 50 12L50 28Q50 40 '));
  assert.ok(wirePath(0, 0, 100, 0, [[50, 0], [50, 40]]).endsWith('L100 0'));
  assert.ok(wirePath(0, 0, 100, 0).startsWith('M0 0C'));
});

test('a route with a new number of points rebuilds the segment lists', () => {
  const store = new WireStore();
  store.set('w', 0, 0, 100, 0, [], false);
  store.layout = false;
  store.set('w', 0, 0, 100, 0, [[50, 40]], false);
  assert.ok(store.layout, 'the bezier and the route have different numbers of points');
  store.layout = false;
  store.set('w', 0, 0, 100, 0, [[50, 60]], false);
  assert.ok(!store.layout, 'a moved point keeps the number of points');
});

test('the value badge sits at the middle of the curve or of the longest segment', () => {
  assert.deepEqual(midpoint(0, 0, 100, 40, []), [50, 20]);
  // Segments of 10, 10 and 20: the badge is at the middle of the last one, not on a route point.
  assert.deepEqual(midpoint(0, 0, 30, 10, [[10, 0], [10, 10]]), [20, 10]);
  // Two equal segments: the first one wins, so the badge is not on the route point between them.
  assert.deepEqual(midpoint(0, 0, 20, 0, [[10, 0]]), [5, 0]);
  assert.deepEqual(midpoint(0, 0, 0, 0, [[0, 0]]), [0, 0]);
});

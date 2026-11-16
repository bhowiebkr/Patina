// The tests of the wire geometry. `npm test` runs them with Node 22.18 or later, which strips the types.

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { anchor, DASHED, FAMILY_SHIFT, HAS_LANES, HOVER, insertAt, instances, LIT, MAX_POINTS, MAX_ROUTE, moveSegment, normals, orthogonal, segments, shape, STALE, STRIDE, WireStore, wirePath, wirePoints } from './wires.ts';
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
  assert.equal(p.length, 2 + 12 + 12);
  near([p[0], p[1], p[12], p[13], p[25]], [[0, 0], [38, 0], [50, 12], [50, 28], [100, 0]]);
  near([p[24]], [[50 + (50 / Math.hypot(50, 40)) * 12, 40 - (40 / Math.hypot(50, 40)) * 12]]);
});

test('a wide ribbon gets a larger corner, and sharp corners stay sharp', () => {
  assert.equal(shape(0, 0, 100, 0, [[50, 0], [50, 80]], undefined, undefined, 40).radius, 30);
  assert.equal(shape(0, 0, 100, 0, [[50, 0], [50, 80]], undefined, undefined, 2).radius, 12);
  assert.equal(shape(0, 0, 100, 0, [[50, 0], [50, 80]], undefined, 'sharp', 40).radius, 0);
});

test('a short segment makes a smaller corner', () => {
  for (const [x, y] of route([50, 0], [50, 4]).slice(0, -1)) assert.ok(x >= 0 && x <= 52 + 1e-5 && y >= -1e-5 && y <= 4 + 1e-5, `${x}, ${y}`);
});

test('a short S-bend of a wide ribbon makes two flat arcs, so no lane folds back', () => {
  const s = shape(0, 0, 200, 6, [], 'orthogonal', undefined, 40);
  const out = new Float32Array(MAX_POINTS * STRIDE);
  const n = wirePoints(0, 0, 200, 6, s, out, 0);
  for (const side of [-20, 20]) {
    for (let k = 1; k < n; k++) assert.ok(out[k * STRIDE] + out[k * STRIDE + 2] * side >= out[(k - 1) * STRIDE] + out[(k - 1) * STRIDE + 2] * side - 1e-3, `side ${side}, point ${k}`);
  }
  near([[out[(n - 1) * STRIDE], out[(n - 1) * STRIDE + 1]]], [[200, 6]]);
  assert.ok(wirePath(0, 0, 200, 6, s).startsWith('M0 0L70 0Q'), 'the SVG path has the same arcs');
  assert.equal(path(100, 40, [], 'orthogonal').length, 2 + 2 * 12, 'a long leg keeps two corners');
});

test('a wire label fits between the two nodes and keeps off the corners', () => {
  assert.equal(anchor(0, 0, 1000, 0, shape(0, 0, 1000, 0, [], 'polyline'))[2], 160);
  assert.equal(anchor(0, 0, 100, 40, shape(0, 0, 100, 40, []))[2], 92, 'the nodes limit the label of a curve');
  // A Z route: the vertical leg is the longest, but the label goes on the longest horizontal leg.
  const z = shape(0, 0, 60, 100, [[40, 0], [40, 100]], 'polyline', 'sharp');
  assert.deepEqual(anchor(0, 0, 60, 100, z), [20, 0, 32]);
  // A corner that turns up into the label takes its radius from each side.
  const up = shape(0, 100, 60, 0, [[40, 100], [40, 0]], 'polyline');
  assert.deepEqual(anchor(0, 100, 60, 0, up), [20, 100, 32 - 2 * up.radius]);
  assert.equal(anchor(0, 0, 60, 100, shape(0, 0, 60, 100, [[40, 0], [40, 100]], 'polyline'))[2], 32, 'a corner that turns down is not in the way');
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
  assert.equal(store.family.length, store.counts.length, 'the families grow with the slots');
  assert.equal(store.hover[21], -1);
});

test('a bus keeps its color family, and the hovered lane is marked', () => {
  const store = new WireStore();
  store.set('bus', 0, 0, 100, 0, shape(0, 0, 100, 0, [], 'polyline'), false, '10', false, 3, 1);
  const flags = 1 | HAS_LANES | (3 << FAMILY_SHIFT);
  assert.deepEqual([...instances(store, false)], [0, 0, 0, flags | LIT, 0, 0, 1, flags | HOVER]);
  store.layout = false;
  store.set('bus', 0, 0, 100, 0, shape(0, 0, 100, 0, [], 'polyline'), false, '10', false, 3, -1);
  assert.ok(store.layout, 'a new hover needs new instances');
  // The wire goes right, so its left side is up: negative y gives a negative distance.
  assert.equal(store.across('bus', 40, -3), -3);
  assert.equal(store.across('bus', 140, 4), 4, 'beyond the end, the nearest segment');
  assert.equal(store.across('none', 0, 0), null);
});

test('a cross-domain wire has the dashed flag, and a change of the flag needs new instances', () => {
  const store = new WireStore();
  const line = shape(0, 0, 100, 0, [], 'polyline');
  store.set('w', 0, 0, 100, 0, line, false, '', false, 0, -1, true);
  assert.deepEqual([...instances(store, false)], [0, 0, 0, DASHED]);
  store.layout = false;
  store.set('w', 0, 0, 100, 0, line, false, '', false, 0, -1, false);
  assert.ok(store.layout);
  assert.deepEqual([...instances(store, false)], [0, 0, 0, 0]);
});

test('the value badge sits at the middle of the curve or of the longest horizontal segment', () => {
  assert.deepEqual(anchor(0, 0, 100, 40, shape(0, 0, 100, 40, [])).slice(0, 2), [50, 20]);
  // Segments of 10, 10 and 20: the badge is at the middle of the last one, not on a route point.
  assert.deepEqual(anchor(0, 0, 30, 10, shape(0, 0, 30, 10, [[10, 0], [10, 10]])).slice(0, 2), [20, 10]);
  // Two equal segments: the first one wins, so the badge is not on the route point between them.
  assert.deepEqual(anchor(0, 0, 20, 0, shape(0, 0, 20, 0, [[10, 0]])).slice(0, 2), [5, 0]);
  assert.deepEqual(anchor(0, 0, 0, 0, shape(0, 0, 0, 0, [[0, 0]])).slice(0, 2), [0, 0]);
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

test('a middle segment of an orthogonal path moves sideways, and the path stays connected', () => {
  assert.deepEqual(segments(0, 0, 100, 0, []), [], 'a straight path has no segment that can move');
  assert.deepEqual(segments(0, 0, 100, 40, []), [{ k: 1, x: 50, y: 20, vertical: true }], 'the segments at the pins stay');
  const moved = moveSegment(0, 0, 100, 40, [], 1, 20);
  assert.deepEqual(orthogonal(0, 0, 100, 40, moved), [[70, 0], [70, 40]], 'the vertical leg moved right');
  const back = [100, 0, 0, 40] as const;
  const legs = segments(...back, []);
  assert.deepEqual(legs.map((s) => s.vertical), [true, false, true]);
  const down = moveSegment(...back, [], legs[1].k, 15);
  assert.deepEqual(orthogonal(...back, down), [[120, 0], [120, 35], [-20, 35], [-20, 40]], 'the horizontal leg moved down');
  assert.deepEqual(moveSegment(...back, [], 0, 15), [], 'a segment at a pin does not move');
  // A move that makes a segment empty merges the runs next to it.
  const flat = moveSegment(0, 0, 100, 40, [[30, 10], [70, 10]], 2, -10);
  assert.deepEqual(flat, [[85, 40]]);
  assert.deepEqual(orthogonal(0, 0, 100, 40, flat), [[85, 0], [85, 40]]);
});

test('sharp corners add no points, and rounded corners add points', () => {
  assert.equal(path(100, 40, [], 'orthogonal', 'sharp').length, 4);
  assert.equal(path(100, 40, [], 'orthogonal').length, 2 + 2 * 12);
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
  near([anchor(0, 0, 100, 0, shape(0, 0, 100, 0, [[50, 40]], 'curved')).slice(0, 2)], [[33.75, 20]]);
});

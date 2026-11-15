// The tests of the wire geometry. `npm test` runs them with Node 22.18 or later, which strips the types.

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { normals, STRIDE } from './wires.ts';

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

import { test } from 'node:test';
import assert from 'node:assert/strict';
import type { Point } from '../api.ts';
import { nudge } from './nudge.ts';
import { GAP, type WireIn } from './route.ts';

/** A wire from x = 0 at y0 to x = 2 * mid at y1, with its vertical segment at x = mid. */
function zig(y0: number, y1: number, net: string, mid = 50): [Point[], WireIn] {
  return [[[0, y0], [mid, y0], [mid, y1], [2 * mid, y1]], { sx: 0, sy: y0, tx: 2 * mid, ty: y1, via: [], net, width: 2 }];
}

function run(list: [Point[], WireIn][]): number[] {
  const paths = list.map(([p]) => p);
  nudge(paths, list.map(([, w]) => w), []);
  return paths.map((p) => p[1][0]);
}

test('short segments next to one long segment each keep a gap from it', () => {
  const x = run([zig(0, 1000, 'n'), ...Array.from({ length: 10 }, (_, k) => zig(100 * k + 10, 100 * k + 60, `u${k}`))]);
  for (const v of x) assert.ok(v >= 10 && v <= 90, `${v} is out of the channel`);
  for (const v of x.slice(1)) assert.ok(Math.abs(v - x[0]) >= 2 + GAP - 0.01, `${v} is too near ${x[0]}`);
});

test('segments in a channel too narrow for the gaps get smaller gaps, but never meet', () => {
  const x = run(Array.from({ length: 5 }, (_, k) => zig(10 * k, 500 + 10 * k, `n${k}`, 20))).sort((a, b) => a - b);
  for (const v of x) assert.ok(v >= 10 - 1e-6 && v <= 30 + 1e-6, `${v} is out of the channel`);
  for (let i = 1; i < x.length; i++) assert.ok(x[i] - x[i - 1] > 2, `${x[i - 1]} and ${x[i]} meet`);
});

test('a wire that turns down before the pin line of another wire keeps the two lines apart', () => {
  // `a` comes up to y = 0 and goes right; `b` goes right at y = -1 and turns down. `c` is between them in the wanted order.
  const list: [Point[], WireIn][] = [
    [[[-40, 19], [50, 19], [50, 0], [200, 0]], { sx: -40, sy: 19, tx: 200, ty: 0, via: [], net: 'a', width: 2 }],
    [[[0, 100], [55, 100], [55, -500], [200, -500]], { sx: 0, sy: 100, tx: 200, ty: -500, via: [], net: 'c', width: 2 }],
    [[[40, -1], [60, -1], [60, 300], [200, 300]], { sx: 40, sy: -1, tx: 200, ty: 300, via: [], net: 'b', width: 2 }],
  ];
  const [a, , b] = run(list);
  assert.ok(b < a, `b at ${b} is not before a at ${a}, so the lines at y = 0 and y = -1 run together`);
});

test('a unit that moves before another unit does not pass a unit that then shares its pin line', () => {
  // `c` turns onto the pin line y = 100 of `b`, and its pin line y = 60 is 5 px from the pin line y = 65 of `a`. A live bool wire is 5 px wide.
  const list = [zig(0, 65, 'a'), zig(100, 10, 'b'), zig(60, 100, 'c')].map(([p, w]): [Point[], WireIn] => [p, { ...w, width: 5 }]);
  const [a, b, c] = run(list);
  assert.ok(b < c, `c at ${c} is before b at ${b}, so the two lines at y = 100 overlap`);
  assert.ok(c < a, `a at ${a} is before c at ${c}, so the lines at y = 60 and y = 65 run 5 px apart`);
});

test('a wire that must cross the pin line of another wire crosses it away from its bends', () => {
  // `a` goes down to its pin line at y = 146; `b` leaves on its pin line at y = 149 and goes up through y = 109, the pin line of `a`.
  const a: Point[] = [[0, 109], [60, 109], [60, 146], [1600, 146]];
  const b: Point[] = [[0, 149], [1270, 149], [1270, 109], [2140, 109], [2140, 124], [2240, 124]];
  const wires: WireIn[] = [
    { sx: 0, sy: 109, tx: 1600, ty: 146, via: [], net: 'a', width: 2 },
    { sx: 0, sy: 149, tx: 2240, ty: 124, via: [], net: 'b', width: 2 },
  ];
  nudge([a, b], wires, []);
  assert.ok(b[1][0] < a[1][0], `b goes up at ${b[1][0]}, after a goes down at ${a[1][0]}, so the pin lines run 3 px apart`);
  assert.ok(Math.abs(b[2][1] - 109) >= 2 + GAP - 0.01, `b at y = ${b[2][1]} is too near the pin line of a`);
});

import { test } from 'node:test';
import assert from 'node:assert/strict';
import type { Point } from './api.ts';
import { GAP, route, Router, type Rect, type WireIn } from './route.ts';

const A: Rect = { l: 0, t: 0, r: 100, b: 100 };
const C: Rect = { l: 400, t: 0, r: 500, b: 100 };
const wire = (sy: number, ty: number, net: string, via: Point[] = []): WireIn => ({ sx: 107, sy, tx: 393, ty, via, net, width: 2 });

function crosses(path: readonly Point[], r: Rect): boolean {
  return path.slice(1).some(([x1, y1], k) => {
    const [x0, y0] = path[k];
    return Math.min(x0, x1) < r.r && Math.max(x0, x1) > r.l && Math.min(y0, y1) < r.b && Math.max(y0, y1) > r.t;
  });
}

test('a route goes around a node between the pins', () => {
  const B: Rect = { l: 200, t: -50, r: 300, b: 150 };
  const out = route([A, B, C], new Map([['w', wire(50, 50, 'a')]]));
  const path = out.get('w')!;
  assert.deepEqual(path[0], [107, 50]);
  assert.deepEqual(path[path.length - 1], [393, 50]);
  assert.equal(crosses(path, B), false);
});

test('a wire with pins less than 1 px apart in y has no jog', () => {
  assert.equal(route([A, C], new Map([['w', wire(50, 50.5, 'a')]])).get('w')!.length, 2);
});

test('parallel wires of two outputs keep a gap', () => {
  const out = route([A, C], new Map([['p', wire(20, 80, 'a')], ['q', wire(40, 100, 'b')]]));
  const x = (id: string) => out.get(id)!.find((p, k, all) => k > 0 && all[k - 1][0] === p[0])![0];
  assert.ok(Math.abs(x('p') - x('q')) >= GAP);
});

test('the wires of one output share a trunk', () => {
  const D: Rect = { l: 400, t: 200, r: 500, b: 300 };
  const out = route([A, C, D], new Map([['near', wire(50, 30, 'a')], ['far', { ...wire(50, 250, 'a') }]]));
  assert.deepEqual(out.get('near')![1], out.get('far')![1]);
});

test('a route goes through its route points', () => {
  const path = route([A, C], new Map([['w', wire(50, 50, 'a', [[250, 300]])]])).get('w')!;
  assert.ok(path.some(([x, y]) => x === 250 && y === 300));
});

test('the router gives the bends after a microtask', async () => {
  const router = new Router();
  let calls = 0;
  router.subscribe(() => calls++);
  router.setObstacles([A, { l: 200, t: -50, r: 300, b: 150 }, C]);
  router.setWire('w', wire(50, 50, 'a'));
  await Promise.resolve();
  const bends = router.bends('w');
  assert.ok(bends && bends.length >= 2);
  assert.equal(calls, 1);
  router.setWire('w', wire(50, 50, 'a'));
  await Promise.resolve();
  assert.equal(router.bends('w'), bends);
});

test('a wire with a route point in a node body has a conflict mark, which goes when the point moves out', async () => {
  const B: Rect = { l: 200, t: 0, r: 300, b: 100 };
  const router = new Router();
  router.setObstacles([A, B, C]);
  router.setWire('w', wire(50, 50, 'a', [[250, 50]]));
  router.setWire('v', wire(70, 70, 'b'));
  await Promise.resolve();
  const at = router.conflict('w');
  assert.ok(at, 'the point in B has no clear path');
  assert.ok(at[0] > 107 && at[0] <= 250, `the mark is on the first segment through a body: ${at}`);
  assert.equal(router.conflict('v'), undefined, 'a wire around B has no mark');
  router.setWire('w', wire(50, 50, 'a', [[250, 200]]));
  await Promise.resolve();
  assert.equal(router.conflict('w'), undefined);
});

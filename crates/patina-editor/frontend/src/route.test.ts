import { test } from 'node:test';
import assert from 'node:assert/strict';
import type { Point } from './api.ts';
import { crossingGaps, junctions } from './marks.ts';
import { GAP, MARGIN, type Rect, type WireIn } from './route.ts';
import { route, Router } from './router.ts';

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

/** The least distance from the inner segments of the path to the body. */
function space(path: readonly Point[], r: Rect): number {
  let min = Infinity;
  for (let k = 1; k + 2 < path.length; k++) {
    const [[x0, y0], [x1, y1]] = [path[k], path[k + 1]];
    const dx = Math.max(r.l - Math.max(x0, x1), Math.min(x0, x1) - r.r, 0);
    const dy = Math.max(r.t - Math.max(y0, y1), Math.min(y0, y1) - r.b, 0);
    min = Math.min(min, Math.max(dx, dy));
  }
  return min;
}

test('a bus goes around a gap that is narrower than the bus, and keeps its clearance', () => {
  const top: Rect = { l: 200, t: -100, r: 300, b: 34 };
  const low: Rect = { l: 200, t: 66, r: 300, b: 200 };
  assert.equal(route([A, top, low, C], new Map([['w', wire(50, 50, 'a')]])).get('w')!.length, 2, 'a plain wire goes through the gap of 32 px');
  const bus = route([A, top, low, C], new Map([['w', { ...wire(50, 50, 'a'), width: 40 }]])).get('w')!;
  assert.ok(bus.some(([, y]) => y < top.t || y > low.b), `the bus goes around the column: ${JSON.stringify(bus)}`);
  for (const r of [A, top, low, C]) assert.ok(space(bus, r) >= MARGIN + 20, `${JSON.stringify(r)} ${JSON.stringify(bus)}`);
});

test('a bus with no path of its clearance routes as a plain wire', () => {
  const box: Rect[] = [
    { l: -200, t: -200, r: 300, b: -150 },
    { l: -200, t: 250, r: 300, b: 300 },
    { l: -200, t: -200, r: -150, b: 300 },
    { l: 200, t: -200, r: 300, b: 34 },
    { l: 200, t: 66, r: 300, b: 300 },
  ];
  assert.equal(route([A, ...box, C], new Map([['w', { ...wire(50, 50, 'a'), width: 40 }]])).get('w')!.length, 2);
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

test('a junction is where the paths of one output part, not at a shared bend or a pin', () => {
  const trunk: Point[] = [[0, 0], [100, 0], [100, 100], [200, 100]];
  assert.deepEqual(junctions([trunk]), [], 'one path has no junction');
  assert.deepEqual(junctions([trunk, [[0, 0], [100, 0], [100, 50], [200, 50]]]), [[100, 50]], 'the branch leaves the side of the trunk');
  assert.deepEqual(junctions([trunk, [[0, 0], [100, 0], [100, -50], [200, -50]]]), [[100, 0]], 'the two paths turn apart at one bend');
  assert.deepEqual(junctions([trunk, [[0, 0], [300, 0]]]), [[100, 0]], 'one path goes on, and the other turns');
  assert.deepEqual(junctions([trunk, [[0, 0], [100, 0], [100, 100], [300, 100]]]), [], 'the same bends and a shared end segment');
  const spur: Point[] = [[0, 0], [50, 0], [50, 50], [50, 0], [100, 0]];
  assert.deepEqual(junctions([spur, [[0, 0], [40, 0], [40, 300], [100, 300]]]), [[40, 0]], 'a path out to a route point and back is no junction');
});

test('the router gives the junction marks of an output on one wire', async () => {
  const D: Rect = { l: 400, t: 200, r: 500, b: 300 };
  const router = new Router();
  router.setObstacles([A, C, D]);
  router.setWire('near', wire(50, 30, 'a'));
  router.setWire('far', wire(50, 250, 'a'));
  router.setWire('lone', wire(80, 80, 'b'));
  await Promise.resolve();
  const [near, far] = [router.junctions('near'), router.junctions('far')];
  assert.equal([near, far].filter(Boolean).length, 1, 'only one wire of the output has the marks');
  assert.equal((near ?? far)!.length, 1);
  assert.equal(router.junctions('lone'), undefined);
  router.removeWire('far');
  await Promise.resolve();
  assert.equal(router.junctions('near') ?? router.junctions('far'), undefined, 'one wire has no junction');
});

test('a wire that crosses a wire of another output gets a gap, away from the bends', () => {
  const across: Point[] = [[0, 50], [200, 50]];
  const down: Point[] = [[100, 0], [100, 100]];
  assert.deepEqual([...crossingGaps([across, down], ['a', 'b'], [2, 2])], [[1, [[100, 50, 4]]]], 'of two wires of one width, the vertical wire goes under');
  assert.deepEqual([...crossingGaps([across, down], ['a', 'b'], [8, 2])], [[1, [[100, 50, 7]]]], 'the narrower wire goes under');
  assert.deepEqual([...crossingGaps([across, down], ['a', 'b'], [2, 8])], [[0, [[100, 50, 7]]]]);
  assert.equal(crossingGaps([across, down], ['a', 'a'], [2, 2]).size, 0, 'the wires of one output join, and do not cross');
  assert.equal(crossingGaps([[[0, 50], [105, 50]], down], ['a', 'b'], [2, 2]).size, 0, 'a crossing near a bend of the upper wire has no gap');
  assert.equal(crossingGaps([[[0, 50], [100, 50]], down], ['a', 'b'], [2, 2]).size, 0, 'a wire that ends on the other wire does not cross it');
});

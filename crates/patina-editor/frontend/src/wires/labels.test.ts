import { test } from 'node:test';
import assert from 'node:assert/strict';
import { Labels } from './labels.ts';
import type { Rect } from './route.ts';

const box = (l: number, t: number, w = 40, h = 13): Rect => ({ l, t, r: l + w, b: t + h });

test('a covered wire label hides, by its rank', () => {
  const labels = new Labels();
  let calls = 0;
  labels.subscribe(() => calls++);
  labels.setObstacles([box(500, 0, 100, 100)]);
  labels.set('bus', box(0, 0), null, 0);
  labels.set('type', box(10, 5), null, 2);
  labels.set('badge', box(200, 0), box(100, 2), 2);
  labels.set('node', box(520, 20), null, 1);
  labels.set('far', box(300, 300), null, 2);
  labels.compute();
  // The bus label covers the type label, a badge covers the label at 100, and a node covers the label at 520.
  assert.deepEqual(['bus', 'type', 'badge', 'node', 'far'].map(labels.hidden), [false, true, false, true, false]);
  labels.set('cover', box(90, 0), null, 2);
  labels.compute();
  assert.equal(labels.hidden('cover'), true);
  // A selected label always shows. A notice goes out only on a change.
  labels.set('cover', box(90, 0), null, -1);
  labels.compute();
  assert.equal(labels.hidden('cover'), false);
  const before = calls;
  labels.compute();
  assert.equal(calls, before);
  labels.remove('bus');
  labels.compute();
  assert.equal(labels.hidden('type'), false);
});

test('a covered badge hides, and a hidden badge does not hide a label', () => {
  const labels = new Labels();
  labels.set('a', null, box(0, 0), 2);
  labels.set('b', null, box(20, 4), 2);
  labels.set('c', box(45, 4), box(200, 0), 2);
  labels.compute();
  assert.deepEqual(['a', 'b'].map(labels.badgeHidden), [false, true]);
  assert.equal(labels.hidden('c'), false);
  // A selected badge always shows, so it hides the badge of a and the label of c.
  labels.set('b', null, box(20, 4), -1);
  labels.compute();
  assert.deepEqual(['a', 'b'].map(labels.badgeHidden), [true, false]);
  assert.equal(labels.hidden('c'), true);
});

test('a badge on a pin or a node body hides, and a selected badge shows', () => {
  const labels = new Labels();
  labels.setObstacles([box(100, 0, 100, 100)]);
  // The pins stick out 6 px from the body, so a badge 3 px from the body covers a pin.
  labels.set('pin', null, box(57, 10), 2);
  labels.set('clear', null, box(50, 40), 2);
  labels.set('body', null, box(150, 60), 2);
  labels.set('selected', null, box(150, 80), -1);
  labels.compute();
  assert.deepEqual(['pin', 'clear', 'body', 'selected'].map(labels.badgeHidden), [true, false, true, false]);
});

test('a badge on a junction dot hides', () => {
  const labels = new Labels();
  labels.setDots([[100, 10]]);
  // The dot has a radius of 4 px, so a badge 3 px from it covers its edge.
  labels.set('dot', null, box(57, 4), 2);
  labels.set('clear', null, box(50, 40), 2);
  labels.set('selected', null, box(80, 4), -1);
  labels.compute();
  assert.deepEqual(['dot', 'clear', 'selected'].map(labels.badgeHidden), [true, false, false]);
});

test('a type label hides with its badge, and a wire name stays', () => {
  const labels = new Labels();
  labels.setObstacles([box(300, 0, 100, 100)]);
  // The badge of a covers the badge of b, and a node covers the badge of the named wire c.
  labels.set('a', box(0, -20), box(0, 0), 2);
  labels.set('b', box(60, -20), box(20, 0), 2);
  labels.set('c', box(200, -20), box(310, 10), 1);
  labels.compute();
  assert.deepEqual(['a', 'b', 'c'].map(labels.badgeHidden), [false, true, true]);
  assert.deepEqual(['a', 'b', 'c'].map(labels.hidden), [false, true, false]);
});

test('a label hides where another wire goes through it, but not on its own wire', () => {
  const labels = new Labels();
  const line = (points: [number, number][]) => ({ points, width: 2 });
  // The wire a goes through the label of b at y 10, and its own label at x 120.
  labels.setLines(new Map([['a', line([[0, 10], [140, 10]])], ['b', line([[0, 40], [140, 40]])]]));
  labels.set('a', box(100, 0), null, 2);
  labels.set('b', box(20, 0), null, 2);
  labels.set('c', box(20, 60), null, 2);
  labels.compute();
  assert.deepEqual(['a', 'b', 'c'].map(labels.hidden), [false, true, false]);
});

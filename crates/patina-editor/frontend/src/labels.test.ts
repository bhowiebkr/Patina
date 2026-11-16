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

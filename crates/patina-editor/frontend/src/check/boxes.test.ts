import assert from 'node:assert/strict';
import { test } from 'node:test';
import { textsMeet } from './boxes.ts';

const box = (left: number, top: number, width: number, height: number) => ({ left, top, right: left + width, bottom: top + height, height });

test('two texts meet when they share width and half of the height', () => {
  assert.equal(textsMeet(box(0, 0, 40, 14), box(30, 4, 40, 14)), true);
  // Two lines 12 px apart share 2 px of their boxes.
  assert.equal(textsMeet(box(0, 0, 40, 14), box(0, 12, 40, 14)), false);
  assert.equal(textsMeet(box(0, 0, 40, 14), box(41, 0, 40, 14)), false);
  assert.equal(textsMeet(box(0, 0, 40, 14), box(39, 0, 40, 14)), false);
});

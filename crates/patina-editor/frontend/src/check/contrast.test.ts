// The tests of the contrast math of the shot. `npm test` runs them.

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { blend, parseColor, ratio } from './contrast.ts';

test('a computed color parses with and without alpha', () => {
  assert.deepEqual(parseColor('rgb(238, 238, 235)'), [238, 238, 235, 1]);
  assert.deepEqual(parseColor('rgba(0, 0, 0, 0.4)'), [0, 0, 0, 0.4]);
  assert.equal(parseColor('color(srgb 1 0 0)'), undefined);
});

test('the ratio of black and white is 21, and of one color 1', () => {
  assert.equal(ratio([0, 0, 0, 1], [255, 255, 255, 1]).toFixed(2), '21.00');
  assert.equal(ratio([48, 48, 48, 1], [48, 48, 48, 1]), 1);
});

test('the muted text of the theme passes on a raised panel, and half alpha fails', () => {
  const raised = parseColor('rgb(48, 48, 48)')!;
  const muted = parseColor('rgb(178, 178, 173)')!;
  assert.ok(ratio(muted, raised) >= 4.5);
  assert.ok(ratio(blend([178, 178, 173, 0.5], raised), raised) < 4.5);
});

// The tests of the text forms of the editor (spec 15.7). `npm test` runs them with Node 22.18 or later.

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { duration } from './api.ts';

test('a node run time uses seconds, milliseconds or microseconds', () => {
  assert.equal(duration(2400), '2.4 s');
  assert.equal(duration(3.14159), '3.14 ms');
  assert.equal(duration(42.4), '42 ms');
  assert.equal(duration(1), '1.00 ms');
  assert.equal(duration(0.85), '850 us');
  assert.equal(duration(0.00042), '0.42 us');
});

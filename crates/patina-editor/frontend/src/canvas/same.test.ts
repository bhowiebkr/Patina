// The tests of the canvas data compare. `npm test` runs them.

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { same } from './same.ts';

test('same compares the items down to the depth', () => {
  const names = ['a', 'b'];
  assert.ok(same({ t: 1, s: { i: { bits: '01', names } } }, { t: 1, s: { i: { bits: '01', names } } }, 3));
  assert.ok(!same({ t: 1, s: { i: { bits: '01', names } } }, { t: 1, s: { i: { bits: '11', names } } }, 3));
  // Below the depth, only the same object is equal.
  assert.ok(!same({ s: { i: { names: ['a'] } } }, { s: { i: { names: ['a'] } } }, 3));
  assert.ok(same([1, 'x'], [1, 'x'], 1));
  assert.ok(!same([1], [1, undefined], 1));
  // A key with no value is not the same as a missing key.
  assert.ok(!same({ x: undefined }, { y: undefined }, 1));
  assert.ok(!same({ x: 1 }, null, 1));
});

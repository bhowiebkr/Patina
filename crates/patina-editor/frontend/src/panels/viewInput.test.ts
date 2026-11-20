// The tests of the input of a View panel (spec 15.8).

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { ViewInput } from './viewInput.ts';

test('a hidden panel sends the last place of each viewport with a size of 0 x 0', () => {
  const input = new ViewInput();
  assert.deepEqual(input.hidden(), []);
  input.push('2 place 9 800 600 10 20 300 200', () => 1);
  input.push('2 place 9 800 600 10 30 300 190', () => 1);
  assert.equal(input.take().events, '2 place 9 800 600 10 30 300 190');
  assert.equal(input.take().events, '');
  assert.deepEqual(input.hidden(), ['2 place 9 800 600 10 30 0 0']);
});

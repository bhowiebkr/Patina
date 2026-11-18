// The tests of the Logs highlight and the fix hints. `npm test` runs them.

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { highlight } from './highlight.ts';
import { hintOf } from './hints.ts';

const kinds = (text: string) => highlight(text).filter(([, k]) => k !== null);

test('the parts of a compile error get their colors', () => {
  assert.deepEqual(kinds('error[E0425]: cannot find value `x` in this scope'), [
    ['error[E0425]', 'sev-err'],
    ['`x`', 'code'],
  ]);
  assert.deepEqual(kinds('  --> nodes/switch/src/lib.rs:4:5'), [
    ['  -->', 'gutter'],
    ['nodes/switch/src/lib.rs:4:5', 'path'],
  ]);
  assert.deepEqual(kinds('12 |     on\n   |     ^^ help'), [
    ['12 |', 'gutter'],
    ['   |', 'gutter'],
    ['^^', 'mark'],
  ]);
  assert.deepEqual(kinds('readout (00000000000000ab): WARN took 12.5 ms "late"'), [
    ['00000000000000ab', 'id'],
    ['WARN', 'sev-warn'],
    ['12.5 ms', 'num'],
    ['"late"', 'str'],
  ]);
  assert.equal(highlight('plain text').length, 1);
  assert.equal(highlight('').length, 0);
  assert.equal(highlight('a1b2 E0425').every(([, k]) => k === null), true, 'digits in a word are no number');
});

test('a common error gets a fix', () => {
  assert.match(hintOf('nodes/switch has no input `on` for the parameter') ?? '', /x of the crossed-out parameter/);
  assert.match(hintOf('error[E0432]: unresolved import `rand`') ?? '', /Add crate/);
  assert.match(hintOf('error[E0308]: mismatched types') ?? '', /Convert/);
  assert.equal(hintOf('Built in 1.2 s'), null);
});

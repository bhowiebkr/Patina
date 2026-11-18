import assert from 'node:assert/strict';
import { test } from 'node:test';
import { focusTarget, splitSelectors } from './focus.ts';

test('a selector list splits only at the commas outside of brackets', () => {
  assert.deepEqual(splitSelectors('.a, .b:is(.c, .d), [x="1,2"]'), ['.a', '.b:is(.c, .d)', '[x="1,2"]']);
});

test('the focus target drops the focus, the pseudo-elements and the parts after the focus', () => {
  assert.equal(focusTarget('.menu button'), undefined);
  assert.deepEqual(focusTarget('.btn:focus-visible'), { target: '.btn', near: false });
  assert.deepEqual(focusTarget('.splitter:focus-visible::before'), { target: '.splitter', near: false });
  assert.deepEqual(focusTarget('.react-flow__edge:focus-visible .wire-hit'), { target: '.react-flow__edge', near: false });
  assert.deepEqual(focusTarget('.recent-row:focus-within .recent-remove'), { target: '.recent-row', near: true });
  assert.deepEqual(focusTarget('.box:has(.code:focus-visible)'), { target: '.box:has(.code)', near: true });
  assert.deepEqual(focusTarget(':focus-visible'), { target: '*', near: false });
});

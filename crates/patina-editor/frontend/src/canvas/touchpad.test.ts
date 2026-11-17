import { test } from 'node:test';
import assert from 'node:assert/strict';
import { isTouchpadScroll } from './touchpad.ts';

const wheel = (wheelDeltaX: number, wheelDeltaY: number, ctrlKey = false, deltaMode = 0) => ({ wheelDeltaX, wheelDeltaY, ctrlKey, deltaMode });

test('a mouse wheel step zooms', () => {
  assert.equal(isTouchpadScroll(wheel(0, 120), Infinity), false);
  assert.equal(isTouchpadScroll(wheel(0, -240), Infinity), false);
  assert.equal(isTouchpadScroll(wheel(0, 3, false, 1), Infinity), false);
});

test('a two-finger touchpad scroll pans on both axes', () => {
  assert.equal(isTouchpadScroll(wheel(0, -37), Infinity), true);
  assert.equal(isTouchpadScroll(wheel(52, 0), Infinity), true);
});

test('a pinch zooms', () => {
  assert.equal(isTouchpadScroll(wheel(0, -37, true), 0), false);
});

test('a step of 120 in a touchpad scroll still pans', () => {
  assert.equal(isTouchpadScroll(wheel(0, 120), 10), true);
  assert.equal(isTouchpadScroll(wheel(0, 0), 10), true);
  assert.equal(isTouchpadScroll(wheel(0, 0), Infinity), false);
});

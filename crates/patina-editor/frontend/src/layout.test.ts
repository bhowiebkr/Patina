// The tests of the dock layout (spec 15.2). `npm test` runs them with Node 22.18 or later.

import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  closePanel,
  defaultLayout,
  layoutText,
  MAX_SIZE,
  MIN_SIZE,
  movePanel,
  openPanel,
  parseLayout,
  resetLayout,
  resizeArea,
  setViewport,
} from './layout.ts';

test('a tab moves before another tab and becomes active', () => {
  const l = movePanel(defaultLayout(), 'logs', 'right', 'inspector');
  assert.deepEqual(l.areas.right, { tabs: ['logs', 'inspector'], active: 'logs', size: 360 });
  assert.deepEqual(l.areas.bottom, { tabs: ['output'], active: 'output', size: 200 });
  assert.equal(movePanel(l, 'logs', 'right', 'logs'), l);
  assert.deepEqual(movePanel(l, 'logs', 'right').areas.right.tabs, ['inspector', 'logs']);
});

test('a closed tab gives the next tab and opens again in its default area', () => {
  let l = closePanel(defaultLayout(), 'output');
  assert.deepEqual(l.areas.bottom, { tabs: ['logs'], active: 'logs', size: 200 });
  l = closePanel(movePanel(l, 'logs', 'left'), 'project');
  assert.deepEqual([l.areas.left.tabs, l.areas.left.active, l.areas.bottom.active], [['logs'], 'logs', null]);
  l = openPanel(l, 'output');
  assert.deepEqual(l.areas.bottom, { tabs: ['output'], active: 'output', size: 200 });
  l = openPanel(l, 'logs');
  assert.deepEqual(l.areas.left.active, 'logs');
});

test('a saved layout reads back, and bad parts are dropped', () => {
  const saved = setViewport(resizeArea(movePanel(defaultLayout(), 'project', 'bottom'), 'left', 300), 'main', { x: 5, y: -2, zoom: 1.5 });
  assert.deepEqual(parseLayout(layoutText(saved)), saved);
  for (const bad of [null, '', '{', '[]', '{"version":2,"areas":{}}']) assert.deepEqual(parseLayout(bad), defaultLayout());
  const odd = parseLayout(
    JSON.stringify({
      version: 1,
      areas: { left: { tabs: ['project', 'debug', 'project', 7], active: 'logs', size: 1e9 }, right: { tabs: ['project', 'logs'], size: 'x' } },
      viewports: { main: { x: 0, y: 0, zoom: 99 }, other: { x: 1, y: 2, zoom: 1 } },
    }),
  );
  assert.deepEqual(odd.areas.left, { tabs: ['project'], active: 'project', size: MAX_SIZE });
  assert.deepEqual(odd.areas.right, { tabs: ['logs'], active: 'logs', size: 360 });
  assert.deepEqual(odd.areas.bottom, { tabs: [], active: null, size: 200 });
  assert.deepEqual(odd.viewports, { other: { x: 1, y: 2, zoom: 1 } });
});

test('sizes are clamped and reset keeps the pan and zoom', () => {
  const l = setViewport(resizeArea(defaultLayout(), 'bottom', 3), 'main', { x: 1, y: 1, zoom: 1 });
  assert.equal(l.areas.bottom.size, MIN_SIZE);
  assert.equal(resizeArea(l, 'bottom', MIN_SIZE), l);
  assert.deepEqual(resetLayout(l), { ...defaultLayout(), viewports: l.viewports });
});

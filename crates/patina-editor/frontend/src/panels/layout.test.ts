// The tests of the dock layout (spec 15.2). `npm test` runs them with Node 22.18 or later.

import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  closePanel,
  defaultLayout,
  dockPanel,
  floatPanel,
  isOpen,
  isShown,
  layoutText,
  MAX_SIZE,
  MIN_SIZE,
  moveFloat,
  movePanel,
  openPanel,
  openView,
  parseLayout,
  railClick,
  railPanels,
  resetLayout,
  resizeArea,
  setViewport,
  showGraph,
  viewTitle,
} from './layout.ts';

test('each view instance opens in a free slot and a close drops it', () => {
  let l = openView(openView(defaultLayout(), 'views/scene'), 'views/scene');
  assert.deepEqual(l.areas.center, { tabs: ['view', 'view2'], active: 'view2', size: 0 });
  assert.deepEqual([viewTitle(l, 'view'), viewTitle(l, 'view2'), viewTitle(l, 'view3')], ['scene', 'scene 2', 'View 3']);
  l = movePanel(l, 'view2', 'bottom');
  assert.equal(l.views.view2, 'views/scene');
  l = closePanel(l, 'view');
  assert.deepEqual(l.views, { view2: 'views/scene' });
  assert.deepEqual(parseLayout(layoutText(l)).views, { view2: 'views/scene' });
  assert.deepEqual(parseLayout(JSON.stringify({ version: 1, areas: {}, views: { view: 'views/a', bad: 'x' } })).views, {});
  l = openView(openView(openView(openView(l, 'a'), 'b'), 'c'), 'd');
  assert.deepEqual(Object.keys(l.views).sort(), ['view', 'view2', 'view3', 'view4']);
  assert.equal(l.views.view2, 'views/scene');
});

test('a tab moves before another tab and becomes active', () => {
  const l = movePanel(defaultLayout(), 'logs', 'right', 'inspector');
  assert.deepEqual(l.areas.right, { tabs: ['logs', 'inspector'], active: 'logs', size: 360 });
  assert.deepEqual(l.areas.bottom, { tabs: ['output', 'debug'], active: 'output', size: 200 });
  assert.equal(movePanel(l, 'logs', 'right', 'logs'), l);
  assert.deepEqual(movePanel(l, 'logs', 'right').areas.right.tabs, ['inspector', 'logs']);
});

test('a closed tab gives the next tab and opens again in its default area', () => {
  let l = closePanel(closePanel(defaultLayout(), 'debug'), 'output');
  assert.deepEqual(l.areas.bottom, { tabs: ['logs'], active: 'logs', size: 200 });
  l = closePanel(movePanel(l, 'logs', 'left'), 'project');
  assert.deepEqual([l.areas.left.tabs, l.areas.left.active, l.areas.bottom.active], [['layers', 'library', 'logs'], 'logs', null]);
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
      areas: { left: { tabs: ['project', 'compare', 'project', 7], active: 'logs', size: 1e9 }, right: { tabs: ['project', 'logs'], size: 'x' } },
      viewports: { main: { x: 0, y: 0, zoom: 99 }, other: { x: 1, y: 2, zoom: 1 } },
    }),
  );
  assert.deepEqual(odd.areas.left, { tabs: ['project', 'layers', 'library'], active: 'project', size: MAX_SIZE }, 'a file without a panel list gets the Layers and Library tabs');
  assert.deepEqual(odd.areas.right, { tabs: ['logs'], active: 'logs', size: 360 });
  assert.deepEqual(odd.areas.bottom, { tabs: [], active: null, size: 200 });
  assert.deepEqual(odd.viewports, { other: { x: 1, y: 2, zoom: 1 } });
});

test('a panel that is newer than the layout file opens in its home area', () => {
  const closed = closePanel(defaultLayout(), 'layers');
  assert.deepEqual(parseLayout(layoutText(closed)).areas.left.tabs, ['project', 'library'], 'a closed panel stays closed');
  const old = JSON.stringify({ version: 1, panels: ['project', 'inspector', 'logs'], areas: { left: { tabs: [] }, right: { tabs: ['inspector'] } } });
  assert.deepEqual(parseLayout(old).areas.left, { tabs: ['layers', 'library'], active: 'layers', size: 260 });
  assert.deepEqual(parseLayout(old).areas.bottom.tabs, ['output', 'debug']);
});

test('sizes are clamped and reset keeps the pan and zoom', () => {
  const l = setViewport(resizeArea(defaultLayout(), 'bottom', 3), 'main', { x: 1, y: 1, zoom: 1 });
  assert.equal(l.areas.bottom.size, MIN_SIZE);
  assert.equal(resizeArea(l, 'bottom', MIN_SIZE), l);
  assert.deepEqual(resetLayout(l), { ...defaultLayout(), viewports: l.viewports });
});

test('the center takes only views, and its Graph tab shows the graph', () => {
  const l = openView(defaultLayout(), 'views/scene');
  assert.equal(movePanel(l, 'logs', 'center'), l, 'a built-in panel does not go in the center');
  const graph = showGraph(l);
  assert.deepEqual(graph.areas.center, { tabs: ['view'], active: null, size: 0 });
  assert.equal(showGraph(graph), graph);
  assert.equal(openPanel(graph, 'view').areas.center.active, 'view');
  assert.deepEqual(parseLayout(layoutText(graph)).areas.center, graph.areas.center);
  const right = movePanel(l, 'view', 'right');
  assert.deepEqual([right.areas.center.tabs, right.areas.center.active, right.areas.right.active], [[], null, 'view']);
  const bad = JSON.stringify({ version: 1, areas: { center: { tabs: ['logs', 'view'], active: 'logs', size: 500 } }, views: { view: 'views/a' } });
  assert.deepEqual(parseLayout(bad).areas.center, { tabs: ['view'], active: null, size: 0 });
});

test('a view slot floats, moves, docks and closes', () => {
  const base = openView(defaultLayout(), 'views/scene');
  assert.deepEqual(floatPanel(defaultLayout(), 'view').floats, {}, 'a slot without a view does not float');
  let l = floatPanel(base, 'view');
  assert.deepEqual(l.floats, { view: null });
  assert.ok(isOpen(l, 'view') && !l.areas.right.tabs.includes('view'));
  assert.deepEqual(openView(l, 'views/b').views, { view: 'views/scene', view2: 'views/b' }, 'a float slot is not free');
  l = moveFloat(l, 'view', { x: 5, y: 6, w: 300, h: 200 });
  assert.deepEqual(parseLayout(layoutText(l)).floats, { view: { x: 5, y: 6, w: 300, h: 200 } });
  const docked = dockPanel(l, 'view');
  assert.deepEqual(docked.floats, {});
  assert.ok(docked.areas.center.tabs.includes('view'));
  assert.equal(docked.views.view, 'views/scene');
  const closed = closePanel(l, 'view');
  assert.deepEqual([closed.floats, closed.views], [{}, {}]);
});

test('a bad float rect or a float without a view does not load', () => {
  const text = (floats: unknown, views: unknown) => JSON.stringify({ version: 1, areas: {}, views, floats });
  assert.deepEqual(parseLayout(text({ view: { x: 1 } }, { view: 'views/a' })).floats, {});
  assert.deepEqual(parseLayout(text({ view: null }, {})).floats, {});
  assert.deepEqual(parseLayout(text({ view: { x: 0, y: 0, w: 1, h: 99999 } }, { view: 'views/a' })).floats, { view: { x: 0, y: 0, w: MIN_SIZE, h: MAX_SIZE } });
});

test('the rail folds and unfolds the left area', () => {
  const start = defaultLayout();
  assert.deepEqual(railPanels(start), ['layers', 'project', 'library']);
  let l = railClick(start, 'project');
  assert.equal(l.folded, true, 'the icon of the active panel folds the area');
  assert.equal(isShown(l, 'project'), false);
  assert.equal(isShown(l, 'inspector'), true);
  assert.equal(parseLayout(layoutText(l)).folded, true);
  assert.equal(railClick(l, 'project').folded, false, 'a second click unfolds it');
  const layers = railClick(l, 'layers');
  assert.equal(layers.folded, false);
  assert.equal(layers.areas.left.active, 'layers');
  assert.equal(openPanel(l, 'project').folded, false, 'an open of a left panel unfolds the area');
  assert.equal(movePanel(l, 'logs', 'left').folded, false);
  assert.equal(movePanel(l, 'logs', 'right').folded, true);
  assert.equal(resetLayout(l).folded, false);
  l = closePanel(start, 'project');
  assert.deepEqual(railPanels(l), ['layers', 'library', 'project'], 'a closed Project panel keeps its icon');
  assert.equal(railClick(l, 'project').areas.left.active, 'project');
  assert.equal(parseLayout(JSON.stringify({ version: 1, areas: {}, folded: 'yes' })).folded, false);
});

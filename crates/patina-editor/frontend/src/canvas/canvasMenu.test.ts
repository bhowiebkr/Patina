import { test } from 'node:test';
import assert from 'node:assert/strict';
import { canvasMenu, type MenuTarget } from './canvasMenu.ts';

const actions = (t: MenuTarget, trusted = true) => canvasMenu(t, trusted).map((e) => (e === 'line' ? '-' : e.action));
const nodes = (count: number, more: Partial<Extract<MenuTarget, { kind: 'nodes' }>> = {}): MenuTarget => ({ kind: 'nodes', count, nodes: count, uses: 0, open: false, pin: false, ...more });

test('the empty canvas adds, pastes, selects, fits and arranges', () => {
  assert.deepEqual(actions({ kind: 'pane' }), ['add', 'paste', '-', 'selectAll', 'fit', 'arrange']);
  assert.deepEqual(actions({ kind: 'pane' }, false), ['selectAll', 'fit']);
});

test('the node entries follow the selection', () => {
  const clip = ['copy', 'cut', 'paste'];
  assert.deepEqual(actions(nodes(1)), ['collapse', 'fit', ...clip, '-', 'delete']);
  assert.deepEqual(actions(nodes(1, { uses: 1, open: true })), ['open', 'collapse', 'expand', 'fit', ...clip, '-', 'delete']);
  // A public pin does not go in a clip.
  assert.deepEqual(actions(nodes(1, { nodes: 0, pin: true })), ['rename', 'fit', 'paste', '-', 'delete']);
  assert.deepEqual(actions(nodes(3, { uses: 1, open: true })), ['collapse', 'expand', 'arrange', 'fit', ...clip, '-', 'delete']);
  assert.deepEqual(actions(nodes(1, { uses: 1, open: true }), false), ['open', 'fit', 'copy']);
});

test('a connection can only be deleted, and only in a trusted project', () => {
  assert.deepEqual(actions({ kind: 'wire' }), ['disconnect']);
  assert.deepEqual(actions({ kind: 'wire' }, false), []);
});

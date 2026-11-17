import { test } from 'node:test';
import assert from 'node:assert/strict';
import { canvasMenu, type MenuTarget } from './canvasMenu.ts';

const actions = (t: MenuTarget, trusted = true) => canvasMenu(t, trusted).map((e) => (e === 'line' ? '-' : e.action));
const nodes = (count: number, more: Partial<Extract<MenuTarget, { kind: 'nodes' }>> = {}): MenuTarget => ({ kind: 'nodes', count, nodes: count, uses: 0, open: false, pin: false, ...more });

test('the empty canvas adds, selects, fits and arranges', () => {
  assert.deepEqual(actions({ kind: 'pane' }), ['add', '-', 'selectAll', 'fit', 'arrange']);
  assert.deepEqual(actions({ kind: 'pane' }, false), ['selectAll', 'fit']);
});

test('the node entries follow the selection', () => {
  assert.deepEqual(actions(nodes(1)), ['collapse', 'fit', '-', 'delete']);
  assert.deepEqual(actions(nodes(1, { uses: 1, open: true })), ['open', 'collapse', 'expand', 'fit', '-', 'delete']);
  assert.deepEqual(actions(nodes(1, { nodes: 0, pin: true })), ['rename', 'fit', '-', 'delete']);
  assert.deepEqual(actions(nodes(3, { uses: 1, open: true })), ['collapse', 'expand', 'arrange', 'fit', '-', 'delete']);
  assert.deepEqual(actions(nodes(1, { uses: 1, open: true }), false), ['open', 'fit']);
});

test('a connection can only be deleted, and only in a trusted project', () => {
  assert.deepEqual(actions({ kind: 'wire' }), ['disconnect']);
  assert.deepEqual(actions({ kind: 'wire' }, false), []);
});

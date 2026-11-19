import { test } from 'node:test';
import assert from 'node:assert/strict';
import { canvasMenu, type MenuTarget } from './canvasMenu.ts';

const actions = (t: MenuTarget, trusted = true) => canvasMenu(t, trusted).map((e) => (e === 'line' ? '-' : e.action));
const nodes = (count: number, more: Partial<Extract<MenuTarget, { kind: 'nodes' }>> = {}): MenuTarget => ({ kind: 'nodes', count, nodes: count, uses: 0, open: false, pin: false, items: 0, ...more });

test('the empty canvas adds, pastes, selects, fits and arranges', () => {
  assert.deepEqual(actions({ kind: 'pane' }), ['add', 'addGroup', 'addNote', 'paste', '-', 'selectAll', 'fit', 'arrange']);
  assert.deepEqual(actions({ kind: 'pane' }, false), ['selectAll', 'fit']);
});

test('the node entries follow the selection', () => {
  const clip = ['copy', 'cut', 'paste'];
  assert.deepEqual(actions(nodes(1)), ['collapse', 'group', 'fit', ...clip, '-', 'delete']);
  assert.deepEqual(actions(nodes(1, { uses: 1, open: true })), ['open', 'collapse', 'group', 'expand', 'fit', ...clip, '-', 'delete']);
  // A public pin does not go in a clip.
  assert.deepEqual(actions(nodes(1, { nodes: 0, pin: true })), ['rename', 'group', 'fit', 'paste', '-', 'delete']);
  assert.deepEqual(actions(nodes(3, { uses: 1, open: true })), ['collapse', 'group', 'expand', 'arrange', 'fit', ...clip, '-', 'delete']);
  // A pin with a group or a note can go in a clip.
  assert.deepEqual(actions(nodes(2, { nodes: 0, items: 1 })), ['group', 'arrange', 'fit', ...clip, '-', 'delete']);
  assert.deepEqual(actions(nodes(1, { uses: 1, open: true }), false), ['open', 'fit', 'copy']);
});

test('a group gets its colors, and a note gets its text', () => {
  const list = canvasMenu({ kind: 'group', count: 1, color: 'teal' }, true);
  const colors = list.flatMap((e) => (e !== 'line' && e.action === 'color' ? [e.label + (e.checked ? '*' : '')] : []));
  assert.deepEqual(colors, ['Gray', 'Amber', 'Green', 'Teal*', 'Violet', 'Pink', 'Copper']);
  assert.deepEqual(actions({ kind: 'group', count: 2, color: null }).filter((a) => a !== 'color'), ['-', 'fit', 'copy', 'cut', 'paste', '-', 'delete']);
  assert.deepEqual(actions({ kind: 'group', count: 1, color: 'teal' }, false), ['fit', 'copy']);
  assert.deepEqual(actions({ kind: 'note', count: 1 }), ['edit', '-', 'fit', 'copy', 'cut', 'paste', '-', 'delete']);
  assert.deepEqual(actions({ kind: 'note', count: 1 }, false), ['fit', 'copy']);
});

test('a connection can only be deleted, and only in a trusted project', () => {
  assert.deepEqual(actions({ kind: 'wire' }), ['disconnect']);
  assert.deepEqual(actions({ kind: 'wire' }, false), []);
});

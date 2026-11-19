// The tests of the groups and the notes of the canvas. `npm test` runs them.

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { GROUP_MARGIN, GROUP_MIN, GROUP_TITLE, groupAround, itemEdit, itemOf, itemsIn, type GroupBoxNode, type NoteNode } from './groups.ts';

test('a canvas ID gives the kind and the file ID of a group or a note', () => {
  assert.deepEqual(itemOf('group:00ff'), { kind: 'group', id: '00ff' });
  assert.deepEqual(itemOf('note:12ab'), { kind: 'note', id: '12ab' });
  assert.equal(itemOf('in:x'), null);
  assert.equal(itemOf('0123456789abcdef'), null);
});

test('an item is in a group only when its full box is in it', () => {
  const group = { l: 0, t: 0, r: 100, b: 100 };
  const items = [
    { id: 'in', l: 10, t: 10, r: 90, b: 90 },
    { id: 'edge', l: 0, t: 0, r: 100, b: 100 },
    { id: 'part', l: 50, t: 50, r: 150, b: 90 },
    { id: 'out', l: 200, t: 0, r: 250, b: 50 },
  ];
  assert.deepEqual(itemsIn([group], items), ['in', 'edge']);
  assert.deepEqual(itemsIn([], items), []);
});

test('a new group has a margin, room for the title bar and the smallest size', () => {
  const g = groupAround([{ l: 100, t: 100, r: 300, b: 200 }, { l: 150, t: 250, r: 200, b: 300 }]);
  assert.deepEqual(g.pos, [100 - GROUP_MARGIN, 100 - GROUP_MARGIN - GROUP_TITLE]);
  assert.deepEqual(g.size, [200 + 2 * GROUP_MARGIN, 200 + 2 * GROUP_MARGIN + GROUP_TITLE]);
  assert.deepEqual(groupAround([{ l: 0, t: 0, r: 1, b: 1 }]).size, [GROUP_MIN[0], 1 + 2 * GROUP_MARGIN + GROUP_TITLE]);
});

test('an item edit uses the place and the size of the view', () => {
  const group: GroupBoxNode = { id: 'group:a', type: 'groupBox', position: { x: 10.4, y: 20.6 }, width: 300.2, height: 150, data: { group: { id: 'a', title: 'T', color: 'amber', pos: [0, 0], size: [1, 1] }, editable: true } };
  assert.deepEqual(itemEdit(group, { color: 'teal' }), { kind: 'SetGroup', group: { id: 'a', title: 'T', color: 'teal', pos: [10, 21], size: [300, 150] } });
  const note: NoteNode = { id: 'note:b', type: 'note', position: { x: 5, y: 6 }, data: { note: { id: 'b', pos: [0, 0], width: 240, text: '# A' }, editable: true } };
  assert.deepEqual(itemEdit(note, { width: 300.7 }), { kind: 'SetNote', note: { id: 'b', pos: [5, 6], width: 301, text: '# A' } });
});

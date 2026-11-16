// The tests of the inner nodes of a subgraph use in place. `npm test` runs them.

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { FRAME_LEFT, FRAME_TOP, frameOf, graphEdit, place, shownFrames, toggleFrame, unplace, type Room } from './frames.ts';

test('the inner nodes get the use prefix and start at the inner corner of the frame', () => {
  const file = {
    format: 1,
    node: [
      { id: 'x', folder: 'nodes/xor', pos: [40, 30] as [number, number] },
      { id: 'n', folder: 'nodes/and', pos: [240, 130] as [number, number] },
    ],
    wire: [{ from: 'x.y', to: 'n.a', points: [[140, 80] as [number, number]], name: 'mid' }],
    input: [{ key: 'a', name: 'a', to: ['x.a', 'n.b'] }],
    output: [{ key: 's', name: 'sum', from: 'x.y' }, { key: 'c', name: 'carry' }],
  };
  const frame = frameOf('u', file);
  assert.deepEqual(frame.nodes.map((n) => [n.entry.id, n.x, n.y]), [
    ['u/x', FRAME_LEFT, FRAME_TOP],
    ['u/n', FRAME_LEFT + 200, FRAME_TOP + 100],
  ]);
  assert.equal(frame.nodes[0].entry.folder, 'nodes/xor');
  assert.deepEqual(frame.wires, [{ from: 'u/x.y', to: 'u/n.a', points: [[FRAME_LEFT + 100, FRAME_TOP + 50]], style: undefined, corners: undefined, name: 'mid' }]);
  assert.deepEqual(frame.inputs, [{ key: 'a', to: 'u/x.a' }, { key: 'a', to: 'u/n.b' }]);
  assert.deepEqual(frame.outputs, [{ key: 's', from: 'u/x.y' }], 'an output with no wire has no inner start');
  assert.deepEqual(frameOf('e', { format: 1 }).nodes, []);
});

test('a use shows its inner nodes until the next toggle, for each graph apart', () => {
  assert.equal(shownFrames('main').size, 0);
  assert.ok(toggleFrame('main', 'u').has('u'));
  assert.equal(shownFrames('alu').size, 0);
  assert.ok(shownFrames('main').has('u'));
  assert.ok(!toggleFrame('main', 'u').has('u'));
});

const room: Room = { use: 'u', x: 100, y: 100, w: 200, h: 100, dx: 300, dy: 200 };

test('an open frame moves the nodes at its right and below it, and keeps the others', () => {
  assert.deepEqual(place([], [5, 6]), [5, 6]);
  assert.deepEqual(place([room], [100, 100]), [100, 100], 'the use stays');
  assert.deepEqual(place([room], [50, 400]), [50, 600]);
  assert.deepEqual(place([room], [300, 0]), [600, 0], 'a node at the right of the use is at the right of the frame');
  assert.deepEqual(place([room], [200, 150]), [350, 250], 'a point across the use moves in proportion');
  assert.deepEqual(place([room], [40, 40]), [40, 40]);
  const two = [room, { use: 'v', x: 250, y: 0, w: 200, h: 80, dx: 100, dy: 50 }];
  for (const p of [[0, 0], [150, 120], [260, 90], [999, -5], [450, 80]] as [number, number][]) {
    const [x, y] = unplace(two, place(two, p));
    assert.ok(Math.abs(x - p[0]) < 1e-9 && Math.abs(y - p[1]) < 1e-9, `${p} goes back`);
  }
});

test('an edit of the view gets graph points', () => {
  assert.deepEqual(graphEdit({ kind: 'MoveNode', id: 'n', pos: [600, 0] }, [room]), { kind: 'MoveNode', id: 'n', pos: [300, 0] });
  assert.deepEqual(graphEdit({ kind: 'MoveNode', id: 'u', pos: [150, 120] }, [room]), { kind: 'MoveNode', id: 'u', pos: [150, 120] }, 'a use does not grow its own place');
  assert.deepEqual(graphEdit({ kind: 'SetRoute', to: 'n.a', points: [[350, 250]] }, [room]), { kind: 'SetRoute', to: 'n.a', points: [[200, 150]] });
  // The node keeps its place in the view after the move of the use in the same edit.
  const group = graphEdit({ kind: 'Group', edits: [{ kind: 'MoveNode', id: 'u', pos: [0, 100] }, { kind: 'MoveNode', id: 'n', pos: [600, 100] }] }, [room]);
  assert.deepEqual(group, { kind: 'Group', edits: [{ kind: 'MoveNode', id: 'u', pos: [0, 100] }, { kind: 'MoveNode', id: 'n', pos: [300, 100] }] });
  assert.deepEqual(place([{ ...room, x: 0 }], [300, 100]), [600, 100]);
  // Two uses move in one edit: the use that moves to the left grows the place of the other use.
  const far = { use: 'v', x: 2000, y: 0, w: 100, h: 100, dx: 200, dy: 0 };
  const both = graphEdit({ kind: 'Group', edits: [{ kind: 'MoveNode', id: 'v', pos: [100, 100] }, { kind: 'MoveNode', id: 'u', pos: [1000, 100] }] }, [room, far]);
  const moved = both.kind === 'Group' ? both.edits.map((e) => (e.kind === 'MoveNode' ? e.pos : null)) : [];
  assert.deepEqual(moved, [[100, 100], [800, 100]]);
  assert.deepEqual(place([{ ...room, x: 800 }, { ...far, x: 100, y: 100 }], [800, 100]), [1000, 100], 'the use shows at the place of the drop');
  const edit = { kind: 'Connect', from: 'a.y', to: 'b.x' } as const;
  assert.equal(graphEdit(edit, [room]), edit);
});

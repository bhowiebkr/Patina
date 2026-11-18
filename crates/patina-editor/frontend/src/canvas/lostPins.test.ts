// The tests of the lost pins. `npm test` runs them.

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { lostPins, wireEnds } from './lostPins.ts';

test('a wire end at a pin key that the node does not have is a lost pin', () => {
  const ends = wireEnds({
    format: 1,
    wire: [
      { from: 'a.out', to: 'u.0000000a' },
      { from: 'u.0000000b', to: 'b.value' },
      { from: 'u.0000000c', to: 'b.other' },
    ],
    input: [{ key: '0000000d', name: 'x', pos: [0, 0], to: ['u.0000000e'] }],
    output: [{ key: '0000000f', name: 'y', pos: [0, 0], from: 'u.0000000b' }],
  });
  assert.deepEqual([...ends.get('u')!].sort(), ['i:0000000a', 'i:0000000e', 'o:0000000b', 'o:0000000c']);
  // The subgraph lost its input 0000000e and its output 0000000c.
  const pins = { inputs: [{ key: '0000000a' }], outputs: [{ key: '0000000b' }] };
  assert.deepEqual(lostPins(pins, ends.get('u')), { inputs: ['0000000e'], outputs: ['0000000c'] });
  assert.equal(lostPins({ inputs: [], outputs: [{ key: 'out' }] }, ends.get('a')), undefined);
  assert.equal(lostPins(pins, undefined), undefined, 'a node with no wires loses nothing');
  assert.deepEqual(lostPins(null, ends.get('a')), { inputs: [], outputs: ['out'] }, 'a missing folder loses all ends');
});

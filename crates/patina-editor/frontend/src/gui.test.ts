// The tests of the node GUI widget lines (spec 4.3). `npm test` runs them with Node 22.18 or later.

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { eventLine, parseTree } from './gui.ts';

test('each widget line gives a widget, and a bad line keeps its index', () => {
  const tree = 'label\tTick\nwheel\tX\nbutton\tReset\ntoggle\tOn\t1\nslider\tCount\t2.5\t0\t10\nslider\tBad\tx\t0\t1';
  assert.deepEqual(parseTree(tree), [
    { kind: 'label', index: 0, text: 'Tick' },
    { kind: 'button', index: 2, text: 'Reset' },
    { kind: 'toggle', index: 3, text: 'On', on: true },
    { kind: 'slider', index: 4, text: 'Count', value: 2.5, low: 0, high: 10 },
  ]);
  assert.equal(eventLine(4, 0.5), '4 0.5');
});

test('fields and dropdowns keep their values, and an event text stays on one line', () => {
  const tree = 'number\tCount\t3\ntext\tName\ta b\ndropdown\tWave\t1\tSine\tSaw\nnumber\tBad\tx';
  assert.deepEqual(parseTree(tree), [
    { kind: 'number', index: 0, text: 'Count', value: 3 },
    { kind: 'text', index: 1, text: 'Name', value: 'a b' },
    { kind: 'dropdown', index: 2, text: 'Wave', selected: 1, options: ['Sine', 'Saw'] },
  ]);
  assert.equal(eventLine(1, 'x\ny'), '1 x y');
});

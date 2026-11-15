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

test('a canvas keeps its good shapes and limits its size', () => {
  const shapes = ['line 0 0 8 8 #010203', 'rect 1 1 -2 2 #000000 1', 'circle 5 5 2.5 #ff0010 1', 'text 1 15 #000000 a  b', 'line 0 0 8 #010203', 'oval 1 1 1 #000000', 'rect 0 0 1 1 red 0'];
  assert.deepEqual(parseTree(`canvas\tDot\t40\t99999\t${shapes.join('\t')}\ncanvas\tBad\t0\t5`), [
    {
      kind: 'canvas',
      index: 0,
      text: 'Dot',
      width: 40,
      height: 4096,
      shapes: [
        { kind: 'line', x1: 0, y1: 0, x2: 8, y2: 8, color: '#010203' },
        { kind: 'circle', x: 5, y: 5, r: 2.5, color: '#ff0010', fill: true },
        { kind: 'text', x: 1, y: 15, color: '#000000', text: 'a  b' },
      ],
    },
  ]);
});

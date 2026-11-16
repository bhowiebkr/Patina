// The tests of the node GUI widget lines (spec 4.3). `npm test` runs them with Node 22.18 or later.

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { eventLine, nest, nodeWidth, parseTree, stepped } from './gui.ts';

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
    { kind: 'number', index: 0, text: 'Count', value: 3, step: 0, unit: '' },
    { kind: 'text', index: 1, text: 'Name', value: 'a b' },
    { kind: 'dropdown', index: 2, text: 'Wave', selected: 1, options: ['Sine', 'Saw'] },
  ]);
  assert.equal(eventLine(1, 'x\ny'), '1 x y');
});

test('a unit, tabs and sections keep their values, and an end line closes the last section', () => {
  const tree = 'number\tGain\t1.5\t0.5\tdB\nnumber\tBad step\t2\t-1\nsection\tMore\t1\ntabs\tPages\t0\tA\tB\nsection\tShut\t0\n\nend\t\nend\t\nend\t\nlabel\tAfter';
  const widgets = parseTree(tree);
  assert.deepEqual(widgets.slice(0, 2), [
    { kind: 'number', index: 0, text: 'Gain', value: 1.5, step: 0.5, unit: 'dB' },
    { kind: 'number', index: 1, text: 'Bad step', value: 2, step: 0, unit: '' },
  ]);
  const [, , more, after] = nest(widgets);
  assert.deepEqual(more, {
    kind: 'section',
    index: 2,
    text: 'More',
    open: true,
    items: [
      { kind: 'tabs', index: 3, text: 'Pages', selected: 0, options: ['A', 'B'] },
      { kind: 'section', index: 4, text: 'Shut', open: false, items: [] },
    ],
  });
  // The empty line of a hidden widget in the closed section keeps its index.
  assert.deepEqual(after, { kind: 'label', index: 9, text: 'After' });
  assert.equal(stepped(0.1, 0.1, 2), 0.3);
  assert.equal(stepped(3, 0.5, -1), 2.5);
});

test('a section deeper than the limit shows its widgets in its parent', () => {
  const tree = [...Array.from({ length: 10 }, (_, i) => `section\tS${i}\t1`), 'label\tDeep', ...Array.from({ length: 10 }, () => 'end\t'), 'label\tTop'].join('\n');
  const blocks = nest(parseTree(tree));
  assert.equal(blocks.length, 2);
  let depth = 0;
  for (let b = blocks[0]; b.kind === 'section'; b = b.items[0]) depth++;
  assert.equal(depth, 8);
  assert.deepEqual(blocks[1], { kind: 'label', index: 21, text: 'Top' });
});

test('value widgets read their data, and a bad line keeps its index', () => {
  const tree = [
    'color\tTint\t#ff0010',
    'plot\tLevel\t0\t2\t0 1.5 NaN',
    'table\tNotes\t2\t1\tNote\tHz\tA\t440\tC',
    'curve\tShape\t\t0.5 0.5\t2 -1',
    'waveform\tWave\t1\t0 0 -0.5 0.5 inf -inf',
    'image\tDot\t1\t1\tAQID/w==',
    'color\tBad\tred',
    'image\tBad\t2\t1\tAQID/w==',
    'curve\tBad\t\t0.5',
    'waveform\tBad\t2\t',
    'table\tBad\t1\tx\tA',
    'waveform\tTrim\t\t0 0\t0.25 1',
    'waveform\tBad trim\t\t\t0.5 0.25',
    'image\tEmpty\t0\t0\t',
  ].join('\n');
  assert.deepEqual(parseTree(tree), [
    { kind: 'color', index: 0, text: 'Tint', value: '#ff0010' },
    { kind: 'plot', index: 1, text: 'Level', low: 0, high: 2, values: [0, 1.5, NaN], axes: null },
    { kind: 'table', index: 2, text: 'Notes', headings: ['Note', 'Hz'], rows: [['A', '440'], ['C']], selected: 1 },
    { kind: 'curve', index: 3, text: 'Shape', points: [[0.5, 0.5], [1, 0]], histogram: [] },
    { kind: 'waveform', index: 4, text: 'Wave', playhead: 1, columns: [[0, 0], [-0.5, 0.5], [NaN, NaN]], trim: null },
    { kind: 'image', index: 5, text: 'Dot', width: 1, height: 1, rgba: 'AQID/w==' },
    { kind: 'waveform', index: 11, text: 'Trim', playhead: null, columns: [[0, 0]], trim: [0.25, 1] },
    { kind: 'image', index: 13, text: 'Empty', width: 0, height: 0, rgba: '' },
  ]);
});

test('a plot can have axes, a scatter plot has points, and a curve can have a histogram', () => {
  const tree = [
    'plot\tResponse\t-1\t2\t1 2\t20\t20000\tHz\tdB',
    'scatter\tDots\t0\t5\t1 2 3 4\t0\t4\ts\t',
    'curve\tLevels\t1 -2 NaN 3\t0.5 0.5',
    'plot\tBad axis\t0\t1\t\tx\t1\t\t',
  ].join('\n');
  assert.deepEqual(parseTree(tree), [
    { kind: 'plot', index: 0, text: 'Response', low: -1, high: 2, values: [1, 2], axes: { x: [20, 20000], units: ['Hz', 'dB'] } },
    { kind: 'scatter', index: 1, text: 'Dots', low: 0, high: 5, values: [1, 2, 3, 4], axes: { x: [0, 4], units: ['s', ''] } },
    { kind: 'curve', index: 2, text: 'Levels', points: [[0.5, 0.5]], histogram: [1, 0, 0, 3] },
  ]);
});

test('icon buttons on adjacent lines share a row, and a progress bar is from 0 to 1', () => {
  const tree = ['icon\tRotate left\trotate-left', 'icon\tMirror\tflip-h', 'progress\tBad\t2', 'icon\tPlay\tplay', 'progress\tFrames\t0.5', 'progress\tBad\tNaN'].join('\n');
  assert.deepEqual(nest(parseTree(tree)), [
    {
      kind: 'icons',
      index: 0,
      items: [
        { kind: 'icon', index: 0, text: 'Rotate left', icon: 'rotate-left' },
        { kind: 'icon', index: 1, text: 'Mirror', icon: 'flip-h' },
      ],
    },
    // The bad line between the buttons keeps its index, so the third button starts a new row.
    { kind: 'icons', index: 3, items: [{ kind: 'icon', index: 3, text: 'Play', icon: 'play' }] },
    { kind: 'progress', index: 4, text: 'Frames', done: 0.5 },
  ]);
});

test('a viewport has a height in the limits', () => {
  assert.deepEqual(parseTree('viewport\tScene\t99999\nviewport\tBad\t0\nviewport\t\t240'), [
    { kind: 'viewport', index: 0, text: 'Scene', height: 4096 },
    { kind: 'viewport', index: 2, text: '', height: 240 },
  ]);
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

test('a canvas sets the node width, and the values and labels do not', () => {
  assert.equal(nodeWidth('label\tA long value that does not fit'), 200);
  assert.equal(nodeWidth('canvas\tLCD\t252\t57\nlabel\tx'), 278);
  assert.equal(nodeWidth('canvas\tBig\t4096\t10'), 480);
});

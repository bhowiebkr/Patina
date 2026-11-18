// The tests of the node GUI widget lines (spec 4.3). `npm test` runs them with Node 22.18 or later.

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { applyTransform, dragTransform, escapeCode, eventLine, movePivot, nest, nodeWidth, parseTree, rampGradient, scaleTo, stepped, transformEvent, unescapeCode } from './gui.ts';

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

test('a code field keeps its line breaks, tabs and backslashes', () => {
  const text = 'LDI 1\n\tOUT ; a\\b\nend\\';
  const line = escapeCode(`${text.slice(0, 5)}\r${text.slice(5)}`);
  assert.equal(line, 'LDI 1\\n\\tOUT ; a\\\\b\\nend\\\\');
  assert.equal(unescapeCode(line), text);
  assert.equal(unescapeCode('a\\qb\\'), 'a\\qb\\');
  const plain = { kind: 'code', index: 0, text: 'Program', value: text, editor: false, current: -1, errors: [] };
  assert.deepEqual(parseTree(`code\tProgram\t${line}`), [plain]);
  // A code editor: the current line, then a line and a text for each error. A bad mark is dropped.
  const editor = parseTree(`code\tProgram\t${line}\t2\t0 bad\\tword\tx y\t1 \ncode\tB\t\t\t`);
  assert.deepEqual(editor, [
    { ...plain, editor: true, current: 2, errors: [[0, 'bad\tword'], [1, '']] },
    { kind: 'code', index: 1, text: 'B', value: '', editor: true, current: -1, errors: [] },
  ]);
  assert.equal(eventLine(0, line), `0 ${line}`);
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
    'image\tBad digit\t1\t1\tAQ!D/w==',
    'image\tNo end\t1\t1\tAQIDBAUG',
    'image\tBad end\t1\t1\tAQID/w=A',
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

test('a color ramp reads its stops in the limits, and a bad blend or color drops the line', () => {
  const tree = ['ramp\tMap\tease\t1\t0 #000000\t1.5 #ffffff', 'ramp\tBad\tsmooth\t0\t0 #000000', 'ramp\tBad\tlinear\t0\t0 #fff', 'ramp\tBad\tlinear\t0.5\t0 #000000'].join('\n');
  assert.deepEqual(parseTree(tree), [
    { kind: 'ramp', index: 0, text: 'Map', blend: 'ease', selected: 1, stops: [[0, '#000000'], [1, '#ffffff']] },
  ]);
});

test('a layer stack reads its rows, keeps a base without an amount, and drops a bad row', () => {
  const tree = ['layers\tLayers\t1\t1 0.35 #e8b080 Edge polish\t0 - #b46438 ', 'layers\tBad\t0\t2 0.5 #000000 X', 'layers\tBad\t0\t1 x #000000 X', 'layers\tBad\tx\t1 0.5 #000000 X'].join('\n');
  assert.deepEqual(parseTree(tree), [
    {
      kind: 'layers',
      index: 0,
      text: 'Layers',
      selected: 1,
      layers: [
        { visible: true, amount: 0.35, color: '#e8b080', name: 'Edge polish' },
        { visible: false, amount: null, color: '#b46438', name: '' },
      ],
    },
  ]);
});

test('a file path reads its dialog kind and drops an unknown kind', () => {
  assert.deepEqual(parseTree('path\tPath\tsave\tout/b c.png\npath\tBad\tdelete\tx'), [{ kind: 'path', index: 0, text: 'Path', pick: 'save', value: 'out/b c.png' }]);
});

test('an asset picker reads its extensions', () => {
  assert.deepEqual(parseTree('asset\tSource\tglb obj\t'), [{ kind: 'asset', index: 0, text: 'Source', extensions: ['glb', 'obj'], value: '' }]);
});

test('a ramp gradient has hard edges for a constant blend and spans the bar for one stop', () => {
  assert.equal(rampGradient([], 'linear'), 'none');
  assert.equal(rampGradient([[0.5, '#ff0000']], 'linear'), 'linear-gradient(to right, #ff0000 0%, #ff0000 50%, #ff0000 100%)');
  assert.equal(
    rampGradient([[0, '#000000'], [0.5, '#ffffff']], 'constant'),
    'linear-gradient(to right, #000000 0%, #000000 0%, #000000 50%, #ffffff 50%, #ffffff 100%)',
  );
});

test('a viewport has a height in the limits', () => {
  assert.deepEqual(parseTree('viewport\tScene\t99999\nviewport\tBad\t0\nviewport\t\t240'), [
    { kind: 'viewport', index: 0, text: 'Scene', height: 4096 },
    { kind: 'viewport', index: 2, text: '', height: 240 },
  ]);
});

test('a canvas keeps its good shapes and limits its size', () => {
  const shapes = ['line 0 0 8 8 #010203', 'rect 1 1 -2 2 #000000 1', 'circle 5 5 2.5 #ff0010 1', 'text 1 15 #000000 a  b', 'line 0 0 8 #010203', 'oval 1 1 1 #000000', 'rect 0 0 1 1 red 0', 'keys'];
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
      keys: true,
    },
  ]);
  assert.equal((parseTree('canvas\tPlain\t8\t8')[0] as { keys: boolean }).keys, false);
});

test('a canvas sets the node width, and the values and labels do not', () => {
  assert.equal(nodeWidth('label\tA long value that does not fit'), 200);
  assert.equal(nodeWidth('canvas\tLCD\t252\t57\nlabel\tx'), 278);
  assert.equal(nodeWidth('canvas\tBig\t4096\t10'), 480);
});

test('a transform line reads its frame, its values and its image, and a bad line drops', () => {
  const tree = [
    'transform\tMove\t100 50\t10 -5\t90\t2 3\t0 1.5\t1\t1\t1\tAQIDBA==',
    'transform\tNo image\t1 1\t0 0\t0\t1 1\t0.5 0.5\t0\t0\t0\t',
    'transform\tBad scale\t1 1\t0 0\t0\t0 1\t0.5 0.5\t0\t0\t0\t',
    'transform\tBad frame\t0 1\t0 0\t0\t1 1\t0.5 0.5\t0\t0\t0\t',
    'transform\tBad image\t1 1\t0 0\t0\t1 1\t0.5 0.5\t0\t1\t1\tAQID',
    'transform\tShort\t1 1\t0 0',
  ].join('\n');
  assert.deepEqual(parseTree(tree), [
    { kind: 'transform', index: 0, text: 'Move', frame: [100, 50], t: { x: 10, y: -5, rotation: 90, sx: 2, sy: 3, px: 0, py: 1, lock: true }, image: { width: 1, height: 1, rgba: 'AQIDBA==' } },
    { kind: 'transform', index: 1, text: 'No image', frame: [1, 1], t: { x: 0, y: 0, rotation: 0, sx: 1, sy: 1, px: 0.5, py: 0.5, lock: false }, image: null },
  ]);
});

test('a transform drag moves, turns and scales around the pivot, and a pivot preset keeps the box', () => {
  const t = { x: 0, y: 0, rotation: 0, sx: 1, sy: 1, px: 0.5, py: 0.5, lock: true };
  const at: [number, number] = [50, 50];
  assert.deepEqual(applyTransform({ ...t, rotation: 90, sx: 2 }, [100, 100], [60, 50]), [50, 70]);
  assert.deepEqual(dragTransform(t, 'move', [0, 0], [10, 4], at, 2, false), { ...t, x: 5, y: 2 });
  assert.deepEqual(dragTransform(t, 'y', [0, 0], [10, 4], at, 2, false), { ...t, y: 2 });
  // A quarter turn clockwise on the screen, and a snap to 15 degrees.
  assert.equal(dragTransform(t, 'rotate', [60, 50], [50, 60], at, 1, false).rotation, 90);
  assert.equal(dragTransform(t, 'rotate', [60, 50], [60, 55], at, 1, true).rotation, 30);
  assert.equal(dragTransform({ ...t, rotation: 170 }, 'rotate', [60, 50], [50, 60], at, 1, false).rotation, -100);
  // A corner twice as far from the pivot doubles the scale. The lock keeps the ratio.
  assert.deepEqual(dragTransform({ ...t, sy: 2 }, 'corner', [60, 60], [70, 70], at, 1, false), { ...t, sx: 2, sy: 4 });
  assert.deepEqual(dragTransform({ ...t, lock: false }, 'corner', [60, 60], [70, 60], at, 1, false), { ...t, lock: false, sx: 2 });
  assert.equal(dragTransform(t, 'corner', [60, 60], [40, 40], at, 1, false).sx, 0.01);
  const turned = { ...t, rotation: 90, sx: 2 };
  const moved = movePivot(turned, [100, 100], [0, 0]);
  assert.deepEqual([moved.px, moved.py], [0, 0]);
  assert.deepEqual(applyTransform(moved, [100, 100], [10, 20]).map(Math.round), applyTransform(turned, [100, 100], [10, 20]).map(Math.round));
  assert.deepEqual(scaleTo({ ...t, sy: 2 }, 3), { ...t, sx: 3, sy: 6 });
  assert.equal(transformEvent({ ...t, x: 1 / 3 }), 'set 0.333 0 0 1 1 0.5 0.5 1');
});

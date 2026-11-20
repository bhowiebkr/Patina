// The tests of the timeline widget line and of its drags (spec 4.3).

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { parseTree } from './gui.ts';
import { clock, curvePath, dragClip, dragPoint, dragRange, quantity, ruler, type Clip } from './timelineData.ts';

// The line of `a_timeline_writes_its_lanes_and_takes_edits` in the SDK tests.
const CLIP = '0 1 0.5 2 #4080c0 4 -1 -1 2 2 -2 -2 3 3 impact 03';
const LINE = `timeline\tTime\t3 1.25 0 1 0 2.4 0.1\t\tclips 0 0 1 Impact\t${CLIP}\tauto 0 0 500 5000 Cutoff\tHz\t0.1 500 1 2000`;

test('a musical timeline keeps beat units and rejects an invalid bar size', () => {
  const [w] = parseTree('timeline\tSong\t128 32 1 0 32 64 0.25 4\t');
  assert.ok(w?.kind === 'timeline');
  assert.equal(w.t.beatsPerBar, 4);
  assert.equal(w.t.playhead, 32);
  for (const beats of [-1, 0.5, 100]) assert.deepEqual(parseTree(`timeline\tSong\t128 32 1 0 32 64 0.25 ${beats}\t`), []);
});

test('a timeline line reads the transport, the clip lanes and the automation lanes', () => {
  assert.deepEqual(parseTree(LINE), [
    {
      kind: 'timeline',
      index: 0,
      text: 'Time',
      t: {
        length: 3,
        playhead: 1.25,
        playing: false,
        looping: true,
        range: [0, 2.4],
        snap: 0.1,
        selected: null,
        lanes: [
          {
            kind: 'clips',
            mute: false,
            solo: false,
            name: 'Impact',
            clips: [{ start: 0, length: 1, offset: 0.5, source: 2, color: '#4080c0', columns: [[-1, -1], [2, 2], [-2, -2], [3, 3]], name: 'impact 03' }],
          },
          { kind: 'auto', mute: false, solo: false, name: 'Cutoff', low: 500, high: 5000, unit: 'Hz', points: [[0.1, 500], [1, 2000]] },
        ],
      },
    },
  ]);
  const [w] = parseTree('timeline\tT\t2 9 1 0 2 -1 0.5\t1 0\tauto 0 1 0 1 A\t\t-1 5 9 0.5\tclips 1 0 0 ');
  assert.ok(w?.kind === 'timeline');
  assert.equal(w.t.playhead, 2);
  assert.deepEqual(w.t.range, [0, 2]);
  assert.deepEqual(w.t.selected, [1, 0]);
  assert.deepEqual(w.t.lanes[0], { kind: 'auto', mute: false, solo: true, name: 'A', low: 0, high: 1, unit: '', points: [[0, 1], [2, 0.5]] });
  assert.deepEqual(w.t.lanes[1], { kind: 'clips', mute: true, solo: false, name: '', clips: [] });
});

test('a bad timeline line drops', () => {
  const bad = [
    'timeline\tT\t3 0 0 0 0 1',
    'timeline\tT\t0 0 0 0 0 1 0\t',
    'timeline\tT\t3 0 0 0 0 1 0\tx',
    'timeline\tT\t3 0 0 0 0 1 0\t\tclips 0 0 2 A\t0 1 0 0 #ffffff 0 a',
    'timeline\tT\t3 0 0 0 0 1 0\t\tclips 0 0 1 A\t0 1 0 0 red 0 a',
    'timeline\tT\t3 0 0 0 0 1 0\t\tclips 0 0 1 A\t0 1 0 0 #ffffff 2 1 1 a',
    'timeline\tT\t3 0 0 0 0 1 0\t\tauto 0 0 5 1 A\t\t',
    'timeline\tT\t3 0 0 0 0 1 0\t\tauto 0 0 0 1 A\t\t1 2 3',
    'timeline\tT\t3 0 0 0 0 1 0\t\tauto 0 0 0 1 A\t',
    'timeline\tT\t3 0 0 0 0 1 0\t\tnotes 0 0 A',
  ];
  for (const line of bad) assert.deepEqual(parseTree(line), [], line);
});

test('the clock, the quantities and the ruler steps have the expected form', () => {
  assert.equal(clock(1.24), '00:00:01.240');
  assert.equal(clock(3725.5), '01:02:05.500');
  assert.equal(clock(-1), '00:00:00.000');
  assert.equal(quantity(2400, 'Hz'), '2.4 kHz');
  assert.equal(quantity(500, 'Hz'), '500 Hz');
  assert.equal(quantity(0.12345, ''), '0.123');
  assert.deepEqual(ruler(3), { step: 0.5, decimals: 1 });
  assert.deepEqual(ruler(1), { step: 0.25, decimals: 2 });
  assert.deepEqual(ruler(40), { step: 5, decimals: 0 });
  assert.deepEqual(ruler(10000), { step: 1800, decimals: 0 });
});

test('a clip drag snaps, stays on the ruler and cannot trim past its source', () => {
  const c: Clip = { start: 1, length: 1, offset: 0.5, source: 2, color: '#ffffff', columns: [], name: 'a' };
  assert.equal(dragClip(c, 'move', 0.53, 3, 0.1).start, 1.5);
  assert.equal(dragClip(c, 'move', 5, 3, 0.1).start, 2);
  assert.equal(dragClip(c, 'move', -5, 3, 0).start, 0);
  // The start trim stops at the start of the source, so the offset becomes 0.
  assert.deepEqual(dragClip(c, 'start', -1, 3, 0.1), { ...c, start: 0.5, length: 1.5, offset: 0 });
  assert.deepEqual(dragClip(c, 'start', 5, 3, 0), { ...c, start: 1.99, length: 0.01, offset: 1.49 });
  // The end trim stops at the end of the source, and a source with no end stops at the ruler end.
  assert.equal(dragClip(c, 'end', 5, 3, 0.1).length, 1.5);
  assert.equal(dragClip({ ...c, source: 0 }, 'end', 5, 3, 0.1).length, 2);
  assert.equal(dragClip(c, 'end', -5, 3, 0).length, 0.01);
});

test('a loop end and a point stay in their limits', () => {
  assert.deepEqual(dragRange([0, 2.4], 1, 0, 3, 0.1), [0, 0.01]);
  assert.deepEqual(dragRange([0, 2.4], 0, 2.6, 3, 0.1), [2.39, 2.4]);
  assert.deepEqual(dragRange([0, 2.4], 1, 2.73, 3, 0.25), [0, 2.75]);
  const points: [number, number][] = [[0.1, 500], [1, 2000], [2, 800]];
  assert.deepEqual(dragPoint(points, 1, 5, 9000, 500, 5000, 3, 0.1), [2, 5000]);
  assert.deepEqual(dragPoint(points, 1, -1, 0, 500, 5000, 3, 0), [0.1, 500]);
  assert.deepEqual(dragPoint([], 0, 1.234, 600.4, 500, 5000, 3, 0.1), [1.2, 600.4]);
});

test('an automation curve is flat outside its points and smooth between them', () => {
  assert.equal(curvePath([[0, 0], [3, 1]], 3, 0, 1), 'M0 1H0C1 1 2 0 3 0H3');
  assert.equal(curvePath([[1, 5]], 4, 0, 10), 'M0 0.5H1H4');
  assert.equal(curvePath([], 2, 0, 1), 'M0 1H2');
  assert.equal(curvePath([[1, 3]], 2, 3, 3), 'M0 0.5H1H2');
});

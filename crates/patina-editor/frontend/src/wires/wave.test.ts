// The tests of the wave on an audio wire. `npm test` runs them.

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { parseWave, wavePath, wireWave } from './wave.ts';
import { STRIDE } from './wires.ts';

/** A line through the x, y pairs, with no normals. */
function line(...xy: number[]): Float32Array {
  const out = new Float32Array((xy.length / 2) * STRIDE);
  for (let k = 0; k < xy.length / 2; k++) out.set([xy[k * 2], xy[k * 2 + 1]], k * STRIDE);
  return out;
}

test('a wave text gives samples from -1 to 1, and a bad text gives none', () => {
  assert.deepEqual(parseWave('100 -50 0'), [1, -0.5, 0]);
  for (const bad of ['', '101', '-101', '1  2', ' 1', '1.5', 'a', '1e2']) assert.deepEqual(parseWave(bad), [], bad);
});

test('a wave goes along the wire, above a wire to the right, and tapers at the two ends', () => {
  assert.equal(wavePath(line(0, 0, 100, 0), 2, [1, 1, 1, 1]), 'M0 0L12.5 -6L37.5 -6L62.5 -6L87.5 -6L100 0');
  assert.equal(wavePath(line(0, 0, 160, 0), 2, [1, -1, 0, 0], 4), 'M0 0L20 -4L60 4L100 0L140 0L160 0');
  // The first sample is near the source, so its taper is small.
  assert.equal(wavePath(line(0, 0, 160, 0), 2, [1, 0, 0, 0, 0, 0, 0, 0]), 'M0 0L10 -3L30 0L50 0L70 0L90 0L110 0L130 0L150 0L160 0');
  // A sample with no size stays on the wire, also after a bend.
  assert.equal(wavePath(line(0, 0, 50, 0, 50, 50), 3, [0, 0, 0, 0]), 'M0 0L12.5 0L37.5 0L50 12.5L50 37.5L50 50');
  // On a wire that goes down, a positive sample is to the right.
  assert.equal(wavePath(line(0, 0, 0, 100), 2, [0, 1, 0, 0]), 'M0 0L0 12.5L6 37.5L0 62.5L0 87.5L0 100');
});

test('a short wire, a wire with no samples or a bad text has no wave', () => {
  assert.equal(wavePath(line(0, 0, 20, 0), 2, [1]), '');
  assert.equal(wavePath(line(0, 0, 100, 0), 2, []), '');
  assert.equal(wireWave(0, 0, 100, 0, { bends: [], radius: 0 }, '50 x'), '');
  assert.match(wireWave(0, 0, 100, 0, { bends: [], radius: 0 }, '50 -50'), /^M0 0L25 -3L75 3L100 0$/);
});

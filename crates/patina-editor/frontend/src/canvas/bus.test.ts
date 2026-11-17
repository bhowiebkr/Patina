import { test } from 'node:test';
import assert from 'node:assert/strict';
import type { KindInfo } from '../api.ts';
import { familyOf, hoveredLane, kindOf, laneAt, laneTip, socketOf, wireBits } from './bus.ts';

const ctrl: KindInfo = { ty: 'u16', lanes: ['HLT', 'MI', 'RI'], label: 'ctrl', color: 'violet' };
const byte: KindInfo = { ty: 'u8', lanes: [], label: null, color: null };

test('an input of a reference type has the kind of the value type', () => {
  assert.equal(kindOf([ctrl, byte], '&u16'), ctrl);
  assert.equal(kindOf([ctrl, byte], '&mut u8'), byte);
  assert.equal(kindOf([ctrl, byte], 'u32'), undefined);
  assert.equal(familyOf(ctrl), 3);
  assert.equal(familyOf(byte), 0);
});

test('a kind with lanes gives off lanes before a run, and the live lanes win', () => {
  assert.equal(wireBits(undefined, ctrl), '000');
  assert.equal(wireBits({ from: 'a.out', text: '5', lanes: { bits: '101', label: 'x' } }, ctrl), '101');
  assert.equal(wireBits({ from: 'a.out', text: 'true' }, undefined), '1');
  assert.equal(wireBits(undefined, byte), '');
});

test('a lane tip has the bus name, the bit, the lane name and the state', () => {
  assert.equal(laneTip('ctrl', ctrl.lanes, '100', 0, true), 'ctrl[2] HLT = 1');
  assert.equal(laneTip('data', [], '01', 1, false), 'data[0]');
  // A live value with another number of lanes uses the bit numbers.
  assert.equal(laneTip('ctrl', ctrl.lanes, '0110', 1, true), 'ctrl[2] = 1');
});

test('the lane under the pointer counts from the left side of the wire', () => {
  // 4 lanes at a pitch of 5: the ribbon is from -10 to 10.
  assert.deepEqual([-9, -4, 1, 9].map((d) => laneAt(d, 4)), [0, 1, 2, 3]);
  assert.equal(laneAt(11.5, 4), 3, 'near the edge');
  assert.equal(laneAt(20, 4), -1);
  assert.equal(laneAt(0, 1), -1, 'one line has no lanes');
});

test('a hover on an output covers all of its wires', () => {
  assert.equal(hoveredLane({ from: 'a.out', to: null, lane: 2 }, 'a.out', 'b.x'), 2);
  assert.equal(hoveredLane({ from: 'a.out', to: 'c.x', lane: 2 }, 'a.out', 'b.x'), -1);
  assert.equal(hoveredLane(null, 'a.out', 'b.x'), -1);
  // The output socket lights for a hover on any of its wires, and an input with no wire never lights.
  assert.equal(hoveredLane({ from: 'a.out', to: 'c.x', lane: 1 }, 'a.out', null), 1);
  assert.equal(hoveredLane({ from: '', to: null, lane: 1 }, '', 'b.x'), -1);
});

test('only a kind with lanes makes a socket, and a value of another width is not shown', () => {
  assert.equal(socketOf(byte, 'a', undefined, false, 'n.out', null), undefined);
  const live = socketOf(ctrl, 'ctrl', { from: 'n.out', text: '5', lanes: { bits: '101', label: 'c' } }, false, 'n.out', null)!;
  assert.deepEqual([live.bits, live.live, live.family], ['101', true, 3]);
  const wide = socketOf(ctrl, 'ctrl', { from: 'n.out', text: '5', lanes: { bits: '0101', label: 'c' } }, false, 'n.out', null)!;
  assert.deepEqual([wide.bits, wide.live], ['000', false]);
});

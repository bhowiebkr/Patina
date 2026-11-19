import { test } from 'node:test';
import assert from 'node:assert/strict';
import type { Pin, Shape } from '../api.ts';
import { socketBar } from './bus.ts';
import { laneOffsets, lanePath, roomOf, symbolLayout, type SymbolLayout } from './symbols.ts';

const pin = (key: string, ty = 'u8'): Pin => ({ key, ty });
const bit = (key: string): Pin => pin(key, 'bool');
/** Each pin of type u8 has 8 lanes, as a kind with 8 lane names gives. */
const eight = (pins: readonly Pin[]) => (key: string) => (pins.find((p) => `${key.slice(2)}` === p.key)?.ty === 'u8' ? 8 : 0);
/** The half height of the pin of a lead on its edge: the bus bar, else the dot of a one-line pin. */
const ext = (n: number) => (n > 1 ? socketBar(n) / 2 : 5.5);

/** Two pins next to each other on one edge never touch: the bars and the dots keep a gap. */
function assertApart(layout: SymbolLayout, what: string) {
  for (const x of [0, layout.width]) {
    const edge = layout.leads.filter((l) => l.end[0] === x).sort((a, b) => a.end[1] - b.end[1]);
    edge.slice(1).forEach((l, i) => {
      const prev = edge[i];
      const gap = l.end[1] - prev.end[1];
      assert.ok(gap >= ext(l.lanes) + ext(prev.lanes) + 1, `${what}: ${prev.key} and ${l.key} are ${gap} px apart`);
    });
  }
  for (const l of layout.leads) {
    assert.ok(l.end[1] - ext(l.lanes) >= -1 && l.end[1] + ext(l.lanes) <= layout.height + 1, `${what}: ${l.key} is in the node`);
  }
}

test('the 8-lane pins of each symbol are bars with lanes, apart from the pins next to them', () => {
  const cases: [Shape, Pin[], Pin[]][] = [
    ['and', [pin('a'), pin('b')], [pin('out')]],
    ['or', [pin('a'), pin('b'), bit('c')], [pin('out')]],
    ['xor', [pin('a'), pin('b')], [pin('out')]],
    ['not', [pin('a')], [pin('out')]],
    ['mux', [pin('a'), pin('b'), bit('sel')], [pin('out')]],
    ['alu', [pin('a'), pin('b'), pin('op')], [pin('out'), bit('zero'), bit('carry')]],
    ['adder', [pin('a'), pin('b'), bit('cin')], [pin('sum'), bit('cout')]],
    ['chip', [pin('d'), bit('load'), bit('clk')], [pin('value')]],
    ['bus', [pin('op')], [bit('b0'), bit('b1'), bit('b2'), bit('b3'), bit('b4'), bit('b5'), bit('b6'), bit('b7')]],
    ['bus', [bit('b0'), bit('b1'), bit('b2'), bit('b3')], [pin('value')]],
    ['bus', [pin('lo'), pin('hi')], [pin('word', 'u16')]],
  ];
  for (const [shape, inputs, outputs] of cases) {
    const lanes = (key: string) => eight(key.startsWith('i:') ? inputs : outputs)(key);
    const layout = symbolLayout({ shape }, 'n', inputs, outputs, lanes);
    const what = `${shape} ${inputs.map((p) => p.key).join(',')}`;
    assertApart(layout, what);
    for (const l of layout.leads) assert.equal(l.lanes, lanes(l.key), `${what}: ${l.key} has its lanes`);
  }
});

test('a symbol with one-line pins keeps the pitch of 20 px', () => {
  const chip = symbolLayout({ shape: 'chip' }, 'reg', [bit('d'), bit('load'), bit('clk')], [bit('q')]);
  assert.deepEqual(chip.leads.filter((l) => l.key.startsWith('i:')).map((l) => l.end[1]), [24, 44, 64]);
  assert.equal(roomOf(0), 20);
  assert.equal(roomOf(8), 50);
});

test('each lane of a lead keeps its offset, also where the lead turns', () => {
  assert.deepEqual(laneOffsets(8).slice(0, 2), [-17.5, -12.5]);
  const straight = { key: 'i:a', end: [0, 30] as [number, number], body: [14, 30] as [number, number], bus: true, leg: false, lanes: 8 };
  assert.equal(lanePath(straight, -17.5), 'M0 12.5L14 12.5');
  assert.equal(lanePath({ ...straight, bend: [40, 30] }, 5), 'M0 35L45 35L19 30');
});

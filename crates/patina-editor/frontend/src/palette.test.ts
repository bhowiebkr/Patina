// The tests of the Add node search (spec 15.2). `npm test` runs them with Node 22.18 or later.

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { matchNodes, matchPin, outputTypeOf, wireType } from './palette.ts';

const nodes = ['show', 'number', 'sum', 'Noise'].map((name) => ({ name }));
const names = (query: string) => matchNodes(nodes, query).map((n) => n.name);

test('an empty query gives all nodes by name', () => {
  assert.deepEqual(names('  '), ['Noise', 'number', 'show', 'sum']);
});

test('a name that starts with the query comes before a name that contains it', () => {
  assert.deepEqual(names('S'), ['show', 'sum', 'Noise']);
  assert.deepEqual(names('um'), ['number', 'sum']);
});

test('the nodes of the project come before the built-in nodes', () => {
  const all = [{ name: 'delay', builtin: true }, { name: 'number' }, { name: 'dice' }];
  assert.deepEqual(matchNodes(all, '').map((n) => n.name), ['dice', 'number', 'delay']);
  assert.deepEqual(matchNodes(all, 'd').map((n) => n.name), ['dice', 'delay']);
});

test('a query with no match gives no node', () => {
  assert.deepEqual(names('xyz'), []);
});

test('a pin matches the wire type, with no reference and with String for &str', () => {
  assert.deepEqual(['f64', '&f64', '&mut Vec<u8>', "&'static str", '&str'].map(wireType), ['f64', 'f64', 'Vec<u8>', 'String', 'String']);
  const add = { inputs: [{ key: 'a', ty: 'f64' }, { key: 'b', ty: 'f64' }], outputs: [{ key: 'out', ty: 'f64' }], generics: [] };
  const show = { inputs: [{ key: 'value', ty: '&f64' }], outputs: [], generics: [] };
  const say = { inputs: [{ key: 'text', ty: '&str' }], outputs: [{ key: 'out', ty: 'String' }], generics: [] };
  const delay = { inputs: [{ key: 'value', ty: 'T' }], outputs: [{ key: 'out', ty: 'T' }], generics: ['T'] };
  assert.equal(matchPin(add, true, 'f64'), 'a', 'from an output, the first input of the type');
  assert.equal(matchPin(show, true, 'f64'), 'value');
  assert.equal(matchPin(say, true, 'String'), 'text');
  assert.equal(matchPin(add, false, '&f64'), 'out', 'from an input, the output of the type');
  assert.equal(matchPin(show, false, 'f64'), undefined, 'a node with no output');
  assert.equal(matchPin(add, true, 'String'), undefined, 'no pin of the type');
  assert.equal(matchPin(delay, true, 'Vec<u8>'), 'value', 'a generic pin joins all types');
  assert.equal(matchPin(delay, false, 'f64'), 'out');
});

test('a pin type matches with no regard to module paths and spaces', () => {
  const say = { inputs: [{ key: 'text', ty: 'std::string::String' }, { key: 'list', ty: 'Vec< ::std::string::String >' }], outputs: [], generics: [] };
  assert.equal(matchPin(say, true, 'String'), 'text');
  assert.equal(matchPin(say, true, '&str'), 'text');
  assert.equal(matchPin(say, true, 'Vec<String>'), 'list');
  assert.equal(matchPin(say, true, 'Vec<u8>'), undefined);
  assert.equal(matchPin(say, true, 'Strings'), undefined, 'only a full name matches');
});

test('a delay output has the type of its input wire, also through a loop of delays', () => {
  const add = { inputs: [], outputs: [{ key: 'out', ty: 'f64' }], generics: [] };
  const delay = { inputs: [{ key: 'value', ty: 'T' }], outputs: [{ key: 'out', ty: 'T' }], generics: ['T'] };
  const pins = (id: string) => (id === 'a' ? add : delay);
  const wires = new Map([['d1.value', 'a.out'], ['d2.value', 'd1.out'], ['d3.value', 'd4.out'], ['d4.value', 'd3.out']]);
  const ty = (from: string) => outputTypeOf(from, pins, (to) => wires.get(to));
  assert.deepEqual(['a.out', 'd1.out', 'd2.out', 'd3.out', 'd5.out'].map(ty), ['f64', 'f64', 'f64', 'T', 'T']);
});

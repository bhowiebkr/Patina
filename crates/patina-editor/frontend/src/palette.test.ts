// The tests of the Add node search (spec 15.2). `npm test` runs them with Node 22.18 or later.

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { matchNodes, matchPin, wireType } from './palette.ts';

const nodes = ['show', 'number', 'sum', 'Noise'].map((name) => ({ name }));
const names = (query: string) => matchNodes(nodes, query).map((n) => n.name);

test('an empty query gives all nodes by name', () => {
  assert.deepEqual(names('  '), ['Noise', 'number', 'show', 'sum']);
});

test('a name that starts with the query comes before a name that contains it', () => {
  assert.deepEqual(names('S'), ['show', 'sum', 'Noise']);
  assert.deepEqual(names('um'), ['number', 'sum']);
});

test('a query with no match gives no node', () => {
  assert.deepEqual(names('xyz'), []);
});

test('a pin matches the wire type, with no reference and with String for &str', () => {
  assert.deepEqual(['f64', '&f64', '&mut Vec<u8>', "&'static str", '&str'].map(wireType), ['f64', 'f64', 'Vec<u8>', 'String', 'String']);
  const add = { inputs: [{ key: 'a', ty: 'f64' }, { key: 'b', ty: 'f64' }], outputs: [{ key: 'out', ty: 'f64' }] };
  const show = { inputs: [{ key: 'value', ty: '&f64' }], outputs: [] };
  const say = { inputs: [{ key: 'text', ty: '&str' }], outputs: [{ key: 'out', ty: 'String' }] };
  assert.equal(matchPin(add, true, 'f64'), 'a', 'from an output, the first input of the type');
  assert.equal(matchPin(show, true, 'f64'), 'value');
  assert.equal(matchPin(say, true, 'String'), 'text');
  assert.equal(matchPin(add, false, '&f64'), 'out', 'from an input, the output of the type');
  assert.equal(matchPin(show, false, 'f64'), undefined, 'a node with no output');
  assert.equal(matchPin(add, true, 'String'), undefined, 'no pin of the type');
});

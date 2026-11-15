// The tests of the Add node search (spec 15.2). `npm test` runs them with Node 22.18 or later.

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { matchNodes } from './palette.ts';

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

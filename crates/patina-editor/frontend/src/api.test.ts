// The tests of the text forms of the editor (spec 15.7). `npm test` runs them with Node 22.18 or later.

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { duration, firstSentence, logKind, nodeProblems, openType, paramEdit, scopeDomains } from './api.ts';

test('a node run time uses seconds, milliseconds or microseconds', () => {
  assert.equal(duration(2400), '2.4 s');
  assert.equal(duration(3.14159), '3.14 ms');
  assert.equal(duration(42.4), '42 ms');
  assert.equal(duration(1), '1.00 ms');
  assert.equal(duration(0.85), '850 us');
  assert.equal(duration(0.00042), '0.42 us');
});

test('a log line has the style of its level', () => {
  assert.deepEqual(['ERROR', 'WARN', 'INFO', 'x'].map(logKind), ['err', 'warn', 'out', 'out']);
});

test('the domains of a subgraph use keep only its own nodes, with the IDs inside the use', () => {
  const domains = { a: 'frame', u: 'event', 'u/b': 'audio', 'u/v/c': 'batch' } as const;
  assert.deepEqual([...scopeDomains(domains, '')], [['a', 'frame'], ['u', 'event']]);
  assert.deepEqual([...scopeDomains(domains, 'u')], [['b', 'audio']]);
  assert.deepEqual([...scopeDomains(domains, 'u/v')], [['c', 'batch']]);
});

test('a generic node with no type is unresolved, not failed', () => {
  const problems = [
    'node d: connect a wire that gives the type of T (spec 4.5). A cycle of generic nodes gives none',
    'node e: connect a wire to the input `value` of the delay (spec 9.2)',
    'node a: no folder',
  ];
  assert.deepEqual(nodeProblems(problems, 'd').map(openType), [true]);
  assert.deepEqual(nodeProblems(problems, 'e').map(openType), [true]);
  assert.deepEqual(nodeProblems(problems, 'a').map(openType), [false]);
});

test('the status bar shows the first sentence of a failure', () => {
  const crash = 'The node process stopped without a request (exit code: 0xc0000005). The node states are lost.\nmore';
  assert.equal(firstSentence(crash), 'The node process stopped without a request (exit code: 0xc0000005).');
  assert.equal(firstSentence('no toolchain\nmore'), 'no toolchain');
});

test('a node GUI writes only the parameters that change, of the nodes in the open graph', () => {
  const graph = { format: 1, node: [{ id: 'a', params: { file: 'x.asm' } }, { id: 'b' }] };
  const set = (id: string, key: string, text: string) => ({ id, key, text });
  assert.equal(paramEdit(graph, [set('a', 'file', 'x.asm'), set('c', 'file', 'y.asm')]), null);
  assert.deepEqual(paramEdit(graph, [set('a', 'file', 'y.asm'), set('a', 'file', 'z.asm')]), { kind: 'SetParam', id: 'a', key: 'file', text: 'z.asm' });
  const both = paramEdit(graph, [set('a', 'file', 'y.asm'), set('b', 'file', 'y.asm')]);
  assert.equal(both?.kind === 'Group' && both.edits.length, 2);
});

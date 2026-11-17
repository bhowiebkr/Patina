// The tests of the conversion of rust-analyzer results. `npm test` runs them with Node 22.18 or later.

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { completionItems, hoverText, kindName, suggestion, toRange } from './lspMap.ts';

const word = { startLineNumber: 3, startColumn: 5, endLineNumber: 3, endColumn: 7 };

test('positions count from 1 in Monaco', () => {
  assert.deepEqual(toRange({ start: { line: 0, character: 0 }, end: { line: 2, character: 4 } }), { startLineNumber: 1, startColumn: 1, endLineNumber: 3, endColumn: 5 });
});

test('the kinds have the names of Monaco', () => {
  assert.equal(kindName(3), 'Function');
  assert.equal(kindName(22), 'Struct');
  assert.equal(kindName(undefined), 'Text');
  assert.equal(kindName(99), 'Text');
});

test('a completion item keeps its edits and its snippet', () => {
  const edit = { range: { start: { line: 2, character: 4 }, end: { line: 2, character: 6 } }, newText: 'push(${1:value})' };
  const s = suggestion({ label: 'push', kind: 2, insertTextFormat: 2, textEdit: edit, tags: [1], additionalTextEdits: [{ ...edit, newText: 'use a;' }] }, word);
  assert.equal(s.insertText, 'push(${1:value})');
  assert.equal(s.snippet, true);
  assert.equal(s.deprecated, true);
  assert.deepEqual(s.range, word);
  assert.deepEqual(s.additionalTextEdits, [{ range: word, text: 'use a;' }]);
  const replace = suggestion({ label: 'x', textEdit: { newText: 'x', insert: edit.range, replace: { ...edit.range, end: { line: 2, character: 9 } } } }, word);
  assert.deepEqual(replace.range, { insert: word, replace: { ...word, endColumn: 10 } });
  const plain = suggestion({ label: 'len', labelDetails: { detail: '()' }, documentation: { kind: 'markdown', value: '**n**' } }, word);
  assert.equal(plain.insertText, 'len');
  assert.equal(plain.snippet, false);
  assert.deepEqual(plain.range, word);
  assert.deepEqual(plain.label, { label: 'len', detail: '()' });
  assert.deepEqual(plain.documentation, { value: '**n**' });
});

test('a completion result is a list or an incomplete list', () => {
  assert.deepEqual(completionItems(null), { items: [], incomplete: false });
  assert.deepEqual(completionItems([{ label: 'a' }]), { items: [{ label: 'a' }], incomplete: false });
  assert.deepEqual(completionItems({ isIncomplete: true, items: [] }), { items: [], incomplete: true });
});

test('hover contents become Markdown', () => {
  assert.equal(hoverText(null), '');
  assert.equal(hoverText({ kind: 'markdown', value: '`u8`' }), '`u8`');
  assert.equal(hoverText([{ language: 'rust', value: 'fn f()' }, 'text']), '```rust\nfn f()\n```\n\n---\n\ntext');
});

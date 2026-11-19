// The tests of the code of the face node of a subgraph (spec 4.3).

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { symbolLayout } from '../canvas/symbols.ts';
import { faceCode } from './faceCode.ts';

test('the face code draws the symbol and puts each public pin of the use', () => {
  const layout = symbolLayout({ shape: 'nand' }, 'gate', [{ key: 'k1', ty: 'bool', name: 'a' }], [{ key: 'k2', ty: 'bool', name: 'y' }], () => 0);
  const code = faceCode('gate-check_look', 'gate', layout, new Map([['i:k1', 'a'], ['o:k2', 'y']]));
  assert.match(code, /pub struct GateCheckLook;/);
  assert.match(code, /#\[node\(free_layout\)\]/);
  assert.match(code, /ui\.pin\("k1", \[-?\d+\.\d+, -?\d+\.\d+\]\); \/\/ input a\n/);
  assert.match(code, /ui\.pin\("k2", \[-?\d+\.\d+, -?\d+\.\d+\]\); \/\/ output y\n/);
  assert.ok(code.includes(`ui.canvas("gate", [${layout.width}.0, ${layout.height}.0])`));
  assert.match(code, /c\.path\("M14 0H42A20 20 0 0 1 42 40H14Z", \[38, 38, 38\], true\);/);
});

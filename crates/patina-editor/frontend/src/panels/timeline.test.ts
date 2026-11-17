import { test } from 'node:test';
import assert from 'node:assert/strict';
import { BLOCK_MS, lanes } from './timeline.ts';

const t = (id: string, start: number, ms: number, thread: number) => ({ id, start, ms, thread });

test('lanes group the calls by domain and thread, sorted by start', () => {
  const domains = { a: 'frame', b: 'frame', c: 'frame', d: 'audio' } as const;
  const out = lanes([t('b', 4, 2, 1), t('a', 1, 2, 0), t('c', 0, 1, 1), t('d', 0, 1, 0), t('e', 0, 5, 0)], domains, 60);
  assert.deepEqual(out.map((l) => l.domain), ['frame', 'audio', 'batch']);
  const [frame, audio, batch] = out;
  assert.deepEqual(frame.rows.map((r) => [r.thread, r.calls.map((c) => c.id)]), [[0, ['a']], [1, ['c', 'b']]]);
  assert.equal(frame.span, 6);
  assert.equal(frame.limit, 1000 / 60);
  assert.equal(audio.limit, BLOCK_MS);
  // A node with no domain, for example in a failed check, shows in the batch lane with no limit.
  assert.deepEqual([batch.limit, batch.span], [null, 5]);
  assert.deepEqual(lanes([], {}, 60), []);
});

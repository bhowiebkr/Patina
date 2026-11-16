// The isolation steps of the self-check (16.1): a node that crashes ends only the node process, and
// a node that does not stop ends at the second Stop. The next run starts a new node process. A node
// that prints without a pause does not stop the window.

import { api, onOutput, type EngineEvent, type View } from './api';
import { A, B, BUILD_MS, RUN_MS } from './check-fixtures';

type Lost = Extract<EngineEvent, { state: 'lost' }>;

export type Helpers = {
  step: (name: string, job: () => Promise<View>) => Promise<View>;
  until: (what: string, ms: number, test: () => boolean) => Promise<void>;
  mark: () => number;
  result: (v: View, from: number, line: string, ms: number) => Promise<unknown>;
  events: EngineEvent[];
  steps: string[];
};

/** Runs the isolation steps on the paused graph of A to B, and returns the last view. */
export async function checkIsolation({ step, until, mark, result, events, steps }: Helpers, v: View): Promise<View> {
  const statusText = () => document.querySelector('.statusbar')?.textContent ?? '';
  const topButton = (label: string) => [...document.querySelectorAll<HTMLButtonElement>('.topbar button')].find((b) => b.textContent === label);
  const failure = async (v: View, from: number) => {
    let lost: Lost | undefined;
    await until('the failure of the node process', RUN_MS, () => {
      lost = events.slice(from).find((e): e is Lost => e.serial === v.serial && e.state === 'lost');
      return lost !== undefined;
    });
    return lost!;
  };

  // The value 66 writes to a bad address, and 55 overflows the stack. The crash handler names the
  // node only on Windows. A pause keeps the edit from a run, so each crash gives one lost event.
  let from = 0;
  const windows = navigator.userAgent.includes('Windows');
  for (const [value, code] of [['66', '0xc0000005'], ['55', '0xc00000fd']]) {
    v = await step(`pause before ${code}`, () => api.pause());
    v = await step(`a value that crashes with ${code}`, () => api.edit({ kind: 'SetParam', id: A, key: 'value', text: value }));
    from = mark();
    v = await step(`run a node that crashes with ${code}`, () => api.run());
    const crash = await failure(v, from);
    for (const text of ['without a request', ...(windows ? [code, `ran node \`${A}\``] : [])]) {
      if (!crash.text.includes(text)) throw new Error(`the crash text has no "${text}": ${crash.text}`);
    }
    await until('the crash in the status bar', RUN_MS, () => statusText().includes('The node process stopped without a request ('));
    from = mark();
    v = await step('undo the crash', () => api.undo());
    if (!v.live) throw new Error('the graph is not live after the crash');
    await result(v, from, '2.5', BUILD_MS);
  }
  steps.push('a crash ends only the node process');

  const runningA = async () => {
    const waitEnd = Date.now() + RUN_MS;
    while ((await api.running()) !== A) {
      if (Date.now() > waitEnd) throw new Error(`no run of node ${A} after ${RUN_MS} ms`);
      await new Promise((r) => setTimeout(r, 20));
    }
  };

  // The value 77 loops and does not read the stop request.
  from = mark();
  v = await step('a node that does not stop', () => api.edit({ kind: 'SetParam', id: A, key: 'value', text: '77' }));
  await runningA();
  await until('the Stop button', RUN_MS, () => topButton('Stop')?.disabled === false);
  topButton('Stop')!.click();
  await until('the node that does not stop in the status bar', RUN_MS, () => statusText().includes(`Node ${A} does not stop`));
  if (topButton('Stop')?.disabled !== false) throw new Error('Stop is off while a node does not stop');
  topButton('Stop')!.click();
  const killed = await failure(v, from);
  const text = `Patina ended the node process, because node \`${A}\` did not stop.`;
  if (!killed.text.startsWith(text)) throw new Error(`the second Stop did not end the node process: ${killed.text}`);
  await until('the end of the node process in the status bar', RUN_MS, () => statusText().includes(text) && !statusText().includes('does not stop'));
  v = await step('undo the endless loop', () => api.undo());
  from = mark();
  v = await step('run in a new node process', () => api.run());
  await result(v, from, '2.5', BUILD_MS);
  steps.push('the second Stop ends the node process');

  // The value 88 prints without a pause until Stop. The window must still answer, and Stop works.
  from = mark();
  v = await step('a node that floods the output', () => api.edit({ kind: 'SetParam', id: A, key: 'value', text: '88' }));
  await runningA();
  let blocks = 0;
  let skipped = false;
  const stopCount = await onOutput((o) => {
    blocks += 1;
    skipped ||= o.text.startsWith('Patina skipped ');
  });
  let slowest = 0;
  let running: string | null = null;
  const floodEnd = Date.now() + 2000;
  while (Date.now() < floodEnd) {
    const t = Date.now();
    running = await api.running();
    slowest = Math.max(slowest, Date.now() - t);
  }
  stopCount();
  if (running !== A || slowest > 1000 || !skipped || blocks > 100) {
    throw new Error(`the flood: node ${running} runs, the slowest ask took ${slowest} ms, skip note ${skipped}, ${blocks} blocks`);
  }
  await until('the Stop button', RUN_MS, () => topButton('Stop')?.disabled === false);
  topButton('Stop')!.click();
  let ended: Extract<EngineEvent, { state: 'ran' }> | undefined;
  await until('the end of the flood', RUN_MS, () => {
    ended = events.slice(from).find((e): e is Extract<EngineEvent, { state: 'ran' }> => e.serial === v.serial && e.state === 'ran');
    return ended !== undefined;
  });
  if (!ended!.ok || ended!.stopped.join() !== B) throw new Error(`Stop did not end the flood: ${JSON.stringify(ended)}`);
  steps.push('the window answers during a flood of output');
  v = await step('undo the flood', () => api.undo());
  from = mark();
  v = await step('run after the flood', () => api.run());
  await result(v, from, '2.5', BUILD_MS);
  return v;
}

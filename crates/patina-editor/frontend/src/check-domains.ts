// The clock domain steps of the self-check: frames until Stop, Step, blocks until Stop, an audio
// delay and the runs of a trigger node (9.1, 9.2, 9.4).

import { api, type EngineEvent, type View } from './api';
import { A, BUILD_MS, EVENT_NODE, RUN_MS, T, fireButton, icon } from './check-fixtures';

export interface DomainCheck {
  step: (name: string, job: () => Promise<View>) => Promise<View>;
  until: (what: string, ms: number, test: () => boolean) => Promise<void>;
  steps: string[];
  /** Marks the start of the events and the output lines to look at, and gives the event index. */
  mark: () => number;
  events: EngineEvent[];
  /** The output lines after the last mark. */
  fresh: () => string[];
  topButton: (label: string) => HTMLButtonElement | undefined;
  statusText: () => string;
}

/** Runs the domain steps with the node of code file `lib`, and returns the last view. */
export async function checkDomains(c: DomainCheck, v: View, lib: string, changedCode: string): Promise<View> {
  const { step, until, steps, mark, events, fresh, topButton, statusText } = c;
  let from = 0;
  // The frame domain (9.1): Run runs the frame node and the show node at each frame until Stop.
  const frameCode = changedCode.replace('#[node]', '#[node(domain = frame)]');
  if (frameCode === changedCode) throw new Error(`the frame check cannot change the code of ${lib}`);
  v = await step('make a frame node', () => api.edit({ kind: 'SetCode', path: lib, text: frameCode }));
  from = mark();
  v = await step('run the frames', () => api.run());
  const ofRun = () => events.slice(from).filter((e) => e.serial === v.serial);
  const frameCount = () => ofRun().filter((e) => e.state === 'frame').length;
  await until('three frames that print 2.5', BUILD_MS, () => {
    const failed = ofRun().find((e) => e.state === 'failed' || ((e.state === 'ran' || e.state === 'frame') && !e.ok));
    if (failed) throw new Error(`the frame run failed: ${JSON.stringify(failed)}`);
    return frameCount() >= 3 && fresh().filter((l) => l === '2.5').length >= 3;
  });
  await until('the frame number in the status bar', RUN_MS, () => /Frame \d+/.test(statusText()));
  topButton('Stop')!.click();
  let frameEnd: EngineEvent | undefined;
  await until('the end of the frames', RUN_MS, () => (frameEnd = ofRun().find((e) => e.state === 'ended')) !== undefined);
  const count = frameCount();
  const end = frameEnd!;
  if (end.state !== 'ended' || end.frames !== count) throw new Error(`the end of the frames does not count ${count} frames: ${JSON.stringify(end)}`);
  await new Promise((r) => setTimeout(r, 100));
  if (frameCount() !== count) throw new Error('a frame ran after Stop');
  // Step (9.1): one click runs one frame of the paused graph.
  await until('the Step button', RUN_MS, () => topButton('Step')?.disabled === false);
  from = mark();
  topButton('Step')!.click();
  await until('the frame of Step', RUN_MS, () => ofRun().some((e) => e.state === 'ended'));
  const stepEnd = ofRun().find((e) => e.state === 'ended');
  if (frameCount() !== 1 || stepEnd?.state !== 'ended' || stepEnd.frames !== 1) throw new Error(`Step did not run one frame: ${JSON.stringify(ofRun())}`);
  // The frame rate (9.1): at 120 FPS, each frame event shows the last of two frames.
  v = await step('set 120 FPS', () => api.frameRate(120));
  if (v.frameRate !== 120) throw new Error(`the view has ${v.frameRate} FPS`);
  await until('120 in the FPS field', RUN_MS, () => document.querySelector<HTMLInputElement>('.fps input')?.value === '120');
  from = mark();
  v = await step('run at 120 FPS', () => api.run());
  const twos = () => ofRun().filter((e) => e.state === 'frame' && e.frames === 2).length;
  await until('three events of two frames', BUILD_MS, () => twos() >= 3);
  topButton('Stop')!.click();
  await until('the end of the 120 FPS run', RUN_MS, () => ofRun().some((e) => e.state === 'ended'));
  const ran = ofRun().reduce((sum, e) => sum + (e.state === 'frame' ? e.frames : 0), 0);
  const fastEnd = ofRun().find((e) => e.state === 'ended');
  if (fastEnd?.state !== 'ended' || fastEnd.frames !== ran) throw new Error(`the end of the 120 FPS run does not count ${ran} frames: ${JSON.stringify(fastEnd)}`);
  // At 100000 FPS, the frames of a tick stop after one tick period (9.1).
  v = await step('set 100000 FPS', () => api.frameRate(100000));
  from = mark();
  v = await step('run at 100000 FPS', () => api.run());
  const many = () => ofRun().filter((e) => e.state === 'frame' && e.frames > 2);
  await until('three events of many frames', BUILD_MS, () => many().length >= 3);
  topButton('Stop')!.click();
  await until('the end of the 100000 FPS run', RUN_MS, () => ofRun().some((e) => e.state === 'ended'));
  const ticks = ofRun().flatMap((e) => (e.state === 'frame' ? [e] : []));
  const slowest = Math.max(...ticks.map((e) => e.runMs));
  if (slowest > 50) throw new Error(`a tick ran its frames for ${slowest} ms`);
  v = await step('set 60 FPS', () => api.frameRate(60));
  v = await step('undo the frame node', () => api.undo());
  steps.push(`frames until Stop: ${count}, at 120 FPS: ${ran}, at 100000 FPS: up to ${Math.max(...ticks.map((e) => e.frames))} in ${slowest.toFixed(1)} ms`);

  // The audio domain (9.3): Run runs the blocks with the frames until Stop. The node does not
  // call `audio::play`, so no sound plays.
  v = await step('make an audio node', () => api.edit({ kind: 'SetCode', path: lib, text: changedCode.replace('#[node]', '#[node(domain = audio)]') }));
  from = mark();
  v = await step('run the blocks', () => api.run());
  await until('three frames of blocks that print 2.5', BUILD_MS, () => {
    const failed = ofRun().find((e) => e.state === 'failed' || ((e.state === 'ran' || e.state === 'frame') && !e.ok));
    if (failed) throw new Error(`the audio run failed: ${JSON.stringify(failed)}`);
    return frameCount() >= 3 && fresh().filter((l) => l === '2.5').length >= 9;
  });
  await until('the frame number of the blocks in the status bar', RUN_MS, () => /Frame \d+/.test(statusText()));
  topButton('Stop')!.click();
  let audioEnd: EngineEvent | undefined;
  await until('the end of the blocks', RUN_MS, () => (audioEnd = ofRun().find((e) => e.state === 'ended')) !== undefined);
  const audioFrames = frameCount();
  // The output pipe can bring the last lines after the end event.
  await new Promise((r) => setTimeout(r, 150));
  const blocks = fresh().filter((l) => l === '2.5').length;
  if (audioEnd?.state !== 'ended' || audioEnd.frames !== audioFrames) throw new Error(`the end of the blocks does not count ${audioFrames} frames: ${JSON.stringify(audioEnd)}`);
  await new Promise((r) => setTimeout(r, 100));
  if (frameCount() !== audioFrames || fresh().filter((l) => l === '2.5').length !== blocks) throw new Error('a block ran after Stop');
  // A delay after an audio node runs in the audio domain, one block late (9.2).
  const delay = 'd000000000000004';
  v = await step('add a delay after the audio node', () =>
    api.edit({
      kind: 'Group',
      edits: [
        { kind: 'AddNode', id: delay, folder: 'delay', builtin: true, pos: [300, 300] },
        { kind: 'Connect', from: `${A}.out`, to: `${delay}.value` },
      ],
    }),
  );
  await until('the audio domain of the delay', RUN_MS, () => icon(delay, 'domain-icon.audio'));
  v = await step('undo the audio delay', () => api.undo());
  v = await step('undo the audio node', () => api.undo());
  steps.push(`blocks until Stop: ${blocks} in ${audioFrames} frames`);

  // The event domain (9.4): Run starts the triggers. The button of a `trigger` node fires one
  // run of its downstream nodes, and a view or the editor can give a text. Stop ends the runs.
  v = await step('make an event node', () => api.edit({ kind: 'SetCode', path: lib, text: EVENT_NODE }));
  v = await step('add a trigger', () =>
    api.edit({
      kind: 'Group',
      edits: [
        { kind: 'AddNode', id: T, folder: 'trigger', builtin: true, pos: [-300, 0] },
        { kind: 'Connect', from: `${T}.out`, to: `${A}.text` },
      ],
    }),
  );
  if (v.problems.length > 0) throw new Error(`problems: ${v.problems.join('; ')}`);
  await until('the trigger kind and event domain icons', RUN_MS, () => icon(T, 'kind-icon.trigger') && icon(T, 'domain-icon.event'));
  from = mark();
  v = await step('run the triggers', () => api.run());
  const eventRuns = () => ofRun().flatMap((e) => (e.state === 'fired' ? [e] : []));
  await until('a run with triggers that listen', BUILD_MS, () => {
    const failed = ofRun().find((e) => e.state === 'failed' || (e.state === 'ran' && !e.ok));
    if (failed) throw new Error(`the event run failed: ${JSON.stringify(failed)}`);
    return ofRun().some((e) => e.state === 'ran' && e.events);
  });
  if (eventRuns().length > 0 || fresh().includes('2.5')) throw new Error('a trigger fired before a click');
  await until('the fire button of the trigger', RUN_MS, () => fireButton()?.disabled === false);
  fireButton()!.click();
  await until('event run 1 that prints 2.5', RUN_MS, () => eventRuns().length >= 1 && fresh().includes('2.5'));
  await api.fire(T, 'abc');
  await until('event run 2 that prints 5.5', RUN_MS, () => eventRuns().length >= 2 && fresh().includes('5.5'));
  const [run1, run2] = eventRuns();
  if (!run1.ok || !run2.ok || run1.id !== 1 || run2.id !== 2 || run1.trigger !== T) throw new Error(`the event runs are wrong: ${JSON.stringify(eventRuns())}`);
  topButton('Stop')!.click();
  await until('the end of the event runs', RUN_MS, () => ofRun().some((e) => e.state === 'ended'));
  if (!(await api.fire(T, '').then(() => false, () => true))) throw new Error('a fire after Stop did not fail');

  // The text "hang" makes a run that does not read the stop request. The second Stop ends it (16.1).
  from = mark();
  v = await step('run the triggers again', () => api.run());
  await until('a second run with triggers that listen', BUILD_MS, () => ofRun().some((e) => e.state === 'ran' && e.events));
  await api.fire(T, 'hang');
  const waitEnd = Date.now() + RUN_MS;
  while ((await api.running()) !== A) {
    if (Date.now() > waitEnd) throw new Error(`no event run of node ${A} after ${RUN_MS} ms`);
    await new Promise((r) => setTimeout(r, 20));
  }
  topButton('Stop')!.click();
  await until('the event node that does not stop in the status bar', RUN_MS, () => statusText().includes(`Node ${A} does not stop`));
  topButton('Stop')!.click();
  await until('the end of the node process after the event run', RUN_MS, () => statusText().includes(`Patina ended the node process, because node \`${A}\` did not stop.`));

  // The text "crash" writes to a bad address in an event run. On Windows, the crash note names the node.
  from = mark();
  v = await step('run the triggers in a new node process', () => api.run());
  await until('a third run with triggers that listen', BUILD_MS, () => ofRun().some((e) => e.state === 'ran' && e.events));
  await api.fire(T, 'crash').catch(() => undefined);
  let lost = '';
  await until('the crash of an event run', RUN_MS, () => {
    const e = ofRun().find((e) => e.state === 'lost');
    lost = e?.state === 'lost' ? e.text : '';
    return lost !== '';
  });
  if (navigator.userAgent.includes('Windows') && !lost.includes(`node \`${A}\` ran`)) throw new Error(`the crash text has no node: ${lost}`);
  v = await step('undo the trigger', () => api.undo());
  v = await step('undo the event node', () => api.undo());
  steps.push('fire a trigger node', 'the second Stop and a crash end an event run');
  return v;
}

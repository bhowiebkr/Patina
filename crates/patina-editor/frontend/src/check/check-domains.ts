// The clock domain steps of the self-check: frames until Stop, Step, blocks until Stop, an audio
// delay and the runs of a trigger node (9.1, 9.2, 9.4).

import { api, type EngineEvent, type View } from '../api';
import { frameStats } from './benchGraph';
import { A, B, BUILD_MS, EVENT_NODE, RUN_MS, T, WAVE_NODE, enterText, fireButton, icon, statusText, topButton, undoCode, type CheckContext } from './check-fixtures';

/**
 * A node drag while the frames run (15.2). The node follows the mouse in each animation frame and
 * does not jump back to its saved place. After the drop it stays, and the move is undone.
 */
async function checkRunDrag({ step, until, steps }: CheckContext, frames: () => number): Promise<View> {
  const node = () => document.querySelector<HTMLElement>(`.react-flow__node[data-id="${A}"]`)!;
  const top = () => node().getBoundingClientRect().top;
  const [top0, grip] = [top(), node().querySelector('.title')!.getBoundingClientRect()];
  const mouse = (dy: number) => ({ bubbles: true, cancelable: true, view: window, button: 0, clientX: grip.left + 10, clientY: grip.top + grip.height / 2 + dy });
  const frame = () => new Promise<number>((r) => requestAnimationFrame(r));
  const realEdit = api.edit;
  let drop: Promise<View> | undefined;
  api.edit = (e) => (drop = realEdit(e));
  const [first, gaps] = [frames(), [] as number[]];
  let [last, most] = [await frame(), 0];
  // The drag starts after the first move, so the node can be a few px behind the mouse.
  try {
    node().querySelector('.title')!.dispatchEvent(new MouseEvent('mousedown', mouse(0)));
    for (let dy = 3; dy <= 150; dy += 3) {
      document.dispatchEvent(new MouseEvent('mousemove', mouse(dy)));
      const now = await frame();
      gaps.push(now - last);
      last = now;
      const moved = top() - top0;
      if (moved < most - 2) throw new Error(`the node jumped back from ${most} px to ${moved} px in a drag while the frames run`);
      most = Math.max(most, moved);
    }
    if (most < 100) throw new Error(`the node moved only ${most} px in a drag of 150 px while the frames run`);
    document.dispatchEvent(new MouseEvent('mouseup', mouse(150)));
    await until('the move edit of the drag while the frames run', RUN_MS, () => drop !== undefined);
  } finally {
    api.edit = realEdit;
  }
  const ran = frames() - first;
  if (ran < 3) throw new Error(`only ${ran} frames ran in the drag`);
  await drop;
  const end = frames();
  await until('three frames after the drop', RUN_MS, () => frames() >= end + 3);
  if (Math.abs(top() - top0 - most) > 2) throw new Error(`the node is ${top() - top0} px from its start after the drop, not ${most} px`);
  const v = await step('undo the drag while the frames run', () => api.undo());
  await until('the node at its start after the undo', RUN_MS, () => Math.abs(top() - top0) <= 2);
  const s = frameStats(gaps);
  steps.push(`node dragged while ${ran} frames ran: animation frames mean ${s.mean} ms, p95 ${s.p95} ms, max ${s.max} ms`);
  return v;
}

/** An audio wire draws the wave of its blocks, and the wave moves with the frames (15.9). */
async function checkWave({ step, until, mark, events }: CheckContext, v: View, lib: string, delay: string): Promise<View> {
  v = await step('make an audio node with a wave', () => api.edit({ kind: 'SetCode', path: lib, text: WAVE_NODE }));
  v = await step('add a delay after the wave node', () =>
    api.edit({
      kind: 'Group',
      edits: [
        // The show node B takes an f64, so it cannot take the blocks.
        { kind: 'Disconnect', from: `${A}.out`, to: `${B}.value` },
        { kind: 'AddNode', id: delay, folder: 'delay', builtin: true, pos: [300, 300] },
        { kind: 'Connect', from: `${A}.out`, to: `${delay}.value` },
      ],
    }),
  );
  const from = mark();
  v = await step('run the wave', () => api.run());
  const wave = () => document.querySelector('.wire-wave:not(.stale)')?.getAttribute('d') ?? '';
  let first = '';
  await until('the wave on the audio wire', BUILD_MS, () => {
    const failed = events.slice(from).find((e) => e.state === 'failed');
    if (failed) throw new Error(`the wave run failed: ${JSON.stringify(failed)}`);
    return (first = wave()) !== '';
  });
  await until('a new wave in a later frame', RUN_MS, () => wave() !== '' && wave() !== first);
  topButton('Stop')!.click();
  await until('the end of the wave run', RUN_MS, () => events.slice(from).some((e) => e.serial === v.serial && e.state === 'ended'));
  v = await step('undo the wave delay', () => api.undo());
  return step('undo the wave node', undoCode);
}

/** Runs the domain steps with the node of code file `lib`, and returns the last view. */
export async function checkDomains(c: CheckContext, v: View, lib: string, changedCode: string): Promise<View> {
  const { step, until, steps, mark, events, fresh } = c;
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
  v = await checkRunDrag(c, frameCount);
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
  // The FPS field of the top bar sends the rate on Enter, and then shows the rate of the view.
  const fps = document.querySelector<HTMLInputElement>('.fps input')!;
  const realRate = api.frameRate;
  let sent: Promise<View> | undefined;
  api.frameRate = (rate) => (sent = realRate(rate));
  enterText(fps, '120');
  await until('the send of the FPS field', RUN_MS, () => sent !== undefined);
  api.frameRate = realRate;
  // The run must not start before the new rate is set.
  await sent;
  await until('120 in the FPS field', RUN_MS, () => fps.value === '120');
  from = mark();
  v = await step('run at 120 FPS', () => api.run());
  if (v.frameRate !== 120) throw new Error(`the view has ${v.frameRate} FPS`);
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
  v = await step('undo the frame node', undoCode);
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
  v = await step('undo the audio node', undoCode);
  steps.push(`blocks until Stop: ${blocks} in ${audioFrames} frames`);
  v = await checkWave(c, v, lib, delay);

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
  v = await step('undo the event node', undoCode);
  steps.push('fire a trigger node', 'the second Stop and a crash end an event run');
  return v;
}

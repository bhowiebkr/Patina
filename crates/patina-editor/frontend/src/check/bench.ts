// The canvas bench of `patina --check <dir> --bench` (docs/quality-plan.md, phase 4). It makes
// graphs of 500 and 2000 nodes and measures the frames of a pan at four zooms and of a node drag,
// and the time of an edit. It writes the numbers to check.json.

import { api, type View } from '../api';
import { benchGraph, frameStats } from './benchGraph';
import { mouseWheel, scale } from './check-fixtures';

const SIZES = [500, 2000];
const ZOOMS = [1, 0.5, 0.3, 0.2];
const FRAMES = 60;
/** The longest time of one measure, in ms. A slow canvas gives fewer frames. */
const MEASURE_MS = 3000;
/** The bench stops before the limit of 180 s of the self-check and writes the numbers that it has. */
const DEADLINE = performance.now() + 140000;
const late = () => performance.now() > DEADLINE;

const frame = () => new Promise<number>((r) => requestAnimationFrame(r));

/** The times between the frames while `each` runs one time in each frame. */
async function frames(each: (i: number) => void): Promise<number[]> {
  const times: number[] = [];
  let last = await frame();
  const end = last + MEASURE_MS;
  for (let i = 0; i < FRAMES && last < end; i++) {
    each(i);
    const now = await frame();
    times.push(now - last);
    last = now;
  }
  return times;
}

function center(): { clientX: number; clientY: number } {
  const box = document.querySelector('.react-flow__pane')!.getBoundingClientRect();
  return { clientX: box.left + box.width / 2, clientY: box.top + box.height / 2 };
}

/** Zooms with mouse wheel events at the center until the zoom is near `target`. */
async function zoomTo(target: number) {
  const pane = document.querySelector('.react-flow__pane')!;
  const { clientX, clientY } = center();
  // A wheel event soon after a pan continues the touchpad scroll (touchpad.ts), so wait for its end.
  await new Promise((r) => setTimeout(r, 200));
  const end = performance.now() + MEASURE_MS;
  for (let i = 0; i < 10 && Math.abs(scale() / target - 1) > 0.02 && performance.now() < end; i++) {
    pane.dispatchEvent(mouseWheel(Math.log2(scale() / target) / 0.002, clientX, clientY));
    await frame();
    await frame();
  }
}

/** A two-finger touchpad scroll step, which pans the view (15.2). */
const panStep = (dx: number, at: { clientX: number; clientY: number }) =>
  new WheelEvent('wheel', { deltaX: dx, deltaY: 0, wheelDeltaX: -dx, wheelDeltaY: 0, ...at, bubbles: true, cancelable: true } as WheelEventInit);

async function benchSize(dir: string, n: number, show: (v: View) => void) {
  show(await api.newProject(dir, `bench${n}`));
  let t = performance.now();
  const v = await api.edit(benchGraph(n));
  const addMs = performance.now() - t;
  show(v);
  while (document.querySelectorAll('.react-flow__node').length < n) {
    if (late()) throw new Error(`the ${n} nodes did not show before the end of the bench`);
    await frame();
  }
  await frame();
  const ms = (x: number) => Math.round(x);
  const result = { nodes: n, viewBytes: JSON.stringify(v).length, addMs: ms(addMs), renderMs: ms(performance.now() - t), zooms: [] as object[], cut: true };
  const pane = document.querySelector('.react-flow__pane')!;
  for (const zoom of ZOOMS) {
    if (late()) return result;
    await zoomTo(zoom);
    const at = center();
    const pan = await frames((i) => pane.dispatchEvent(panStep(i < FRAMES / 2 ? 12 : -12, at)));
    result.zooms.push({ zoom, scale: Math.round(scale() * 100) / 100, pan: frameStats(pan) });
  }
  if (late()) return result;
  // A drag of the node nearest to the center, at the zoom 1.
  await zoomTo(1);
  const { clientX, clientY } = center();
  const titles = [...document.querySelectorAll<HTMLElement>('.react-flow__node .title')];
  const far = titles.map((e) => Math.hypot(e.getBoundingClientRect().left - clientX, e.getBoundingClientRect().top - clientY));
  const near = titles[far.indexOf(Math.min(...far))];
  const grip = near.getBoundingClientRect();
  const mouse = (dx: number) => ({ bubbles: true, cancelable: true, view: window, button: 0, clientX: grip.left + 10 + dx, clientY: grip.top + grip.height / 2 });
  near.dispatchEvent(new MouseEvent('mousedown', mouse(0)));
  const drag = await frames((i) => document.dispatchEvent(new MouseEvent('mousemove', mouse(3 * (i + 1)))));
  document.dispatchEvent(new MouseEvent('mouseup', mouse(3 * FRAMES)));
  await new Promise((r) => setTimeout(r, 1000));
  // One move edit: the time to the new view, and to the frame that shows it.
  const id = near.closest<HTMLElement>('.react-flow__node')!.dataset.id!;
  t = performance.now();
  const moved = await api.edit({ kind: 'MoveNode', id, pos: [0, -400] });
  const editMs = performance.now() - t;
  show(moved);
  await frame();
  const editFrameMs = performance.now() - t;
  return { ...result, editMs: ms(editMs), editFrameMs: ms(editFrameMs), drag: frameStats(drag), cut: false };
}

export async function runBench(dir: string, show: (v: View) => void) {
  const steps: string[] = [];
  const bench = [];
  try {
    for (const n of SIZES) {
      if (late()) break;
      bench.push(await benchSize(dir, n, show));
      steps.push(`bench ${n}`);
    }
    await api.checkDone({ ok: true, steps, bench });
  } catch (e) {
    await api.checkDone({ ok: false, steps, bench, error: String(e) });
  }
}

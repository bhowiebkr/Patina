// The bench of one frame path. It measures three scenarios, then runs the checks of spec 15.8.
// It drives the page with DevTools input, so the window stays off-screen and never takes the
// desktop focus. Each check records `ok` and the facts that decide it.

import { invoke } from '@tauri-apps/api/core';
import { emitTo, listen } from '@tauri-apps/api/event';
import { mode, shell } from './main';
import { page, percentile, syncClock, Viewport } from './viewport';

/** A probe rectangle that shows the scene has this part of colored pixels, or more. */
const SEEN = 0.08;
/** A probe rectangle that the panels cover has this part of colored pixels, or less. */
const GONE = 0.02;

const params = new URLSearchParams(location.search);
const measureMs = Number(params.get('measure') ?? 2) * 1000;

type Stats = Record<string, number | string | number[]>;
type Check = { ok: boolean; [fact: string]: unknown };

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const round = (v: number) => Math.round(v * 100) / 100;

function frames(n: number): Promise<void> {
  return new Promise((resolve) => {
    let left = n;
    const step = () => (--left <= 0 ? resolve() : requestAnimationFrame(step));
    requestAnimationFrame(step);
  });
}

const stats = () => invoke<Stats>('read_stats');
const cdp = (method: string, params: object) => invoke('cdp', { method, params });

function mouse(type: string, x: number, y: number, button = 'left') {
  const buttons = type === 'mouseReleased' ? 0 : button === 'left' ? 1 : 2;
  return cdp('Input.dispatchMouseEvent', { type, x, y, button, buttons, clickCount: 1 });
}

async function key() {
  await cdp('Input.dispatchKeyEvent', { type: 'keyDown', key: 'a', code: 'KeyA', text: 'a' });
  await cdp('Input.dispatchKeyEvent', { type: 'keyUp', key: 'a', code: 'KeyA' });
}

/** The captures of the current check. */
export let captures = 0;
export const seen = (values: number[]) => values.every((v) => v >= SEEN);
const gone = (values: number[]) => values.every((v) => v <= GONE);

/**
 * Returns the part of each rectangle that shows the scene. A null rectangle gives 0. It captures
 * again, 10 times at most, until `done` accepts the values: on the Intel UHD 770 a capture can
 * come before the compositor shows the swapchain image.
 */
export async function capture(label: string, name: string, rects: (number[] | null)[], done: (values: number[]) => boolean) {
  const valid = rects.filter((r): r is number[] => r !== null);
  let values: number[] = [];
  for (let i = 0; i < 10; i++) {
    if (i > 0) await sleep(100);
    captures++;
    const raw = await invoke<number[]>('probe', { label, rects: valid, name });
    values = rects.map((r) => (r ? round(raw[valid.indexOf(r)]) : 0));
    if (done(values)) break;
  }
  return values;
}

const probe = (check: string, rects: (number[] | null)[], done: (values: number[]) => boolean) =>
  capture('main', `${mode}-${check}`, rects, done);

/** The rectangle of an element in physical pixels. */
function physOf(id: string): number[] {
  const r = document.getElementById(id)!.getBoundingClientRect();
  const dpr = devicePixelRatio;
  return [r.left, r.top, r.width, r.height].map((v) => Math.round(v * dpr));
}

/** Drags in a circle in the middle of a viewport, one move in each animation frame. */
async function drag(vp: Viewport, ms: number) {
  const r = vp.el.getBoundingClientRect();
  const [cx, cy] = [r.left + r.width / 2, r.top + r.height / 2];
  await mouse('mousePressed', cx, cy);
  const end = performance.now() + ms;
  for (let i = 0; performance.now() < end; i++) {
    await frames(1);
    await mouse('mouseMoved', cx + 80 * Math.cos(i * 0.1), cy + 80 * Math.sin(i * 0.1));
  }
  await mouse('mouseReleased', cx, cy);
}

async function scenario(name: string, setup: () => void) {
  setup();
  await frames(10);
  page.reset();
  await invoke('scenario_begin');
  await drag(shell.vp1!, measureMs);
  const secs = (performance.now() - page.since) / 1000;
  const rect = shell.vp1!.rect;
  await invoke('scenario_end', {
    name,
    page: {
      size: rect ? `${rect[2]}x${rect[3]}` : 'none',
      page_fps: round(page.ticks / secs),
      draw_fps: round(page.draws / secs),
      upload_ms_p95: percentile(page.uploadMs, 0.95),
      page_latency_ms_p50: percentile(page.latencyMs, 0.5),
      page_latency_ms_p95: percentile(page.latencyMs, 0.95),
      page_latency_samples: page.latencyMs.length,
      inputs: page.inputs,
    },
  });
}

async function until(test: () => boolean, ms: number): Promise<boolean> {
  const end = performance.now() + ms;
  while (!test()) {
    if (performance.now() > end) return false;
    await sleep(20);
  }
  return true;
}

const checks: Record<string, () => Promise<Check>> = {
  async composite() {
    const [vp, panel] = await probe('composite', [shell.vp1!.rect, physOf('inspector')], ([v, p]) => v >= SEEN && p <= GONE);
    return { ok: vp >= SEEN && panel <= GONE, vp, panel };
  },

  async menu() {
    // A right click opens the menu over the scene. The probe strip has no text: the colored
    // edges of ClearType text count as scene pixels.
    const r = shell.vp1!.el.getBoundingClientRect();
    const [x, y] = [r.left + r.width * 0.4, r.top + r.height * 0.3];
    await mouse('mousePressed', x, y, 'right');
    await mouse('mouseReleased', x, y, 'right');
    await frames(8);
    const opened = document.getElementById('menu')!.hidden === false;
    const m = physOf('menu');
    const strip = [m[0] + Math.round(m[2] * 0.7), m[1] + 12, Math.round(m[2] * 0.25), m[3] - 24];
    const [open] = await probe('menu', [strip], gone);
    shell.menu(false);
    await frames(8);
    const [closed] = await probe('menu-closed', [strip], seen);
    return { ok: opened && open <= GONE && closed >= SEEN, opened, open, closed };
  },

  async splitter() {
    const before = shell.vp1!.rect!;
    const s = document.getElementById('split')!.getBoundingClientRect();
    const [x, y] = [s.left + s.width / 2, s.top + s.height / 2];
    await mouse('mousePressed', x, y);
    await mouse('mouseMoved', x - 200, y);
    await mouse('mouseReleased', x - 200, y);
    await frames(8);
    const after = shell.vp1!.rect!;
    const [vp] = await probe('splitter', [after], seen);
    shell.setRightWidth(300);
    const shrunk = before[2] - after[2];
    return { ok: shrunk > 150 && vp >= SEEN, shrunk, vp };
  },

  async resize() {
    const configured = Number((await stats()).surfaces_configured);
    await invoke('set_size', { width: 1200, height: 700 });
    const resized = await until(() => innerWidth < 1300, 2000);
    await frames(8);
    const small = shell.vp1!.rect;
    const [vp] = await probe('resize', [small], seen);
    const reconfigured = Number((await stats()).surfaces_configured) - configured;
    await invoke('set_size', { width: 1536, height: 864 });
    await until(() => innerWidth > 1500, 2000);
    await frames(8);
    // Only the native path has a window surface to configure again.
    return { ok: resized && (mode !== 'native' || reconfigured > 0) && vp >= SEEN, vp, reconfigured, small };
  },

  async hide_restore() {
    const rect = shell.vp1!.rect;
    shell.hide();
    await frames(8);
    const [hidden] = await probe('hidden', [rect], gone);
    shell.restore();
    await frames(8);
    const [restored] = await probe('restored', [shell.vp1!.rect], seen);
    return { ok: hidden <= GONE && restored >= SEEN, hidden, restored };
  },

  async close_reopen() {
    const rect = shell.vp1!.rect;
    await shell.close();
    await frames(8);
    const [closed] = await probe('closed', [rect], gone);
    await shell.reopen();
    await frames(8);
    const [reopened] = await probe('reopened', [shell.vp1!.rect], seen);
    return { ok: closed <= GONE && reopened >= SEEN, closed, reopened };
  },

  async float_dock() {
    const rect = shell.vp1!.rect;
    let floatProbe: { fraction: number; captures: number; rect: number[] | null } | null = null;
    const stop = await listen<{ fraction: number; captures: number; rect: number[] | null }>('float-probe', (e) => (floatProbe = e.payload));
    await shell.float();
    await frames(8);
    const [left] = await probe('floating', [rect], gone);
    const floated = await until(() => floatProbe !== null, 5000);
    stop();
    await emitTo('float-1', 'dock');
    const docked = await until(() => shell.vp1State === 'docked', 3000);
    await frames(8);
    const [back] = await probe('docked', [shell.vp1!.rect], seen);
    const fp = floatProbe as { fraction: number; captures: number; rect: number[] | null } | null;
    const float = fp?.fraction ?? 0;
    return { ok: floated && docked && left <= GONE && float >= SEEN && back >= SEEN, left, float, back, float_captures: fp?.captures ?? 0, float_rect: fp?.rect };
  },

  async focus() {
    await cdp('Emulation.setFocusEmulationEnabled', { enabled: true });
    const name = document.getElementById('name') as HTMLInputElement;
    const keys0 = Number((await stats()).keys);
    name.focus();
    await key();
    await frames(4);
    const keys1 = Number((await stats()).keys);
    shell.vp1!.el.focus();
    await key();
    await frames(4);
    const keys2 = Number((await stats()).keys);
    const typed = name.value.length === 'Cube grid'.length + 1;
    return { ok: typed && keys1 === keys0 && keys2 === keys1 + 1, typed, field_keys: keys1 - keys0, viewport_keys: keys2 - keys1 };
  },

  async display_scale() {
    const base = devicePixelRatio;
    const result: Check = { ok: true, base };
    for (const dpr of [2, 1]) {
      await invoke('set_dpr', { dpr });
      const changed = await until(() => Math.abs(devicePixelRatio - dpr) < 0.01, 2000);
      await frames(8);
      const rect = shell.vp1!.rect;
      const [vp] = await probe(`dpr${dpr}`, [rect], seen);
      result[`dpr${dpr}`] = { vp, rect };
      result.ok &&= changed && vp >= SEEN;
    }
    await invoke('set_dpr', { dpr: base });
    await until(() => Math.abs(devicePixelRatio - base) < 0.01, 2000);
    await frames(8);
    return result;
  },

  async two_views() {
    shell.setTab('vp2');
    await frames(8);
    const [vp1, vp2] = await probe('two-views', [shell.vp1!.rect, shell.vp2!.rect], seen);
    shell.setTab('inspector');
    return { ok: vp1 >= SEEN && vp2 >= SEEN, vp1, vp2 };
  },

  async device_lost() {
    const count = () => stats().then((s) => (s.recovery_ms as number[]).length);
    const before = await count();
    await invoke('lose_device');
    let recovered = false;
    for (let i = 0; i < 50 && !recovered; i++) {
      await sleep(100);
      recovered = (await count()) > before;
    }
    await frames(8);
    const [vp] = await probe('device-lost', [shell.vp1!.rect], seen);
    const s = await stats();
    return { ok: recovered && vp >= SEEN, vp, recovery_ms: s.recovery_ms, device_lost: s.device_lost };
  },
};

export async function runBench() {
  await sleep(1000);
  const clock = await syncClock();
  await scenario('one', () => shell.setTab('inspector'));
  await scenario('two', () => shell.setTab('vp2'));
  await scenario('full', () => shell.setFull(true));
  shell.setFull(false);
  shell.setTab('inspector');
  await frames(8);
  const results: Record<string, Check> = {};
  for (const [name, run] of Object.entries(checks)) {
    captures = 0;
    try {
      results[name] = { ...(await run()), captures };
    } catch (e) {
      results[name] = { ok: false, error: String(e), captures };
    }
  }
  const s = await stats();
  const info = {
    mode,
    dpr: devicePixelRatio,
    adapter: s.adapter,
    surface_format: s.surface_format,
    clock_offset_ms: round(clock),
    checks: results,
  };
  for (const vp of [shell.vp1, shell.vp2]) if (vp?.registered) await vp.unregister();
  const next = await invoke<string | null>('run_done', { info });
  if (next) location.href = next;
}

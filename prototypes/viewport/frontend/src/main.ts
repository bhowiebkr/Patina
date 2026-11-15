// The editor shell of the viewport prototype: a top bar with a menu, a left dock with viewport 1,
// a splitter, a right dock with viewport 2 and an inspector, and a status bar. A float window
// shows one viewport alone. With `bench=1` the page runs the bench of its frame path.

import { invoke } from '@tauri-apps/api/core';
import { emitTo, listen } from '@tauri-apps/api/event';
import { capture, captures, runBench, seen } from './bench';
import { page, percentile, syncClock, Viewport } from './viewport';
import './style.css';

const params = new URLSearchParams(location.search);
export const mode = params.get('mode') ?? 'native';
export const bench = params.get('bench') === '1';
const floatId = Number(params.get('float') ?? 0);

function reportError(message: string) {
  invoke('page_error', { message }).catch(() => {});
}
window.addEventListener('error', (e) => reportError(String(e.message)));
window.addEventListener('unhandledrejection', (e) => reportError(String(e.reason)));
document.body.classList.toggle('copy', mode !== 'native');

const $ = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T;

/** The state of the dock layout. The bench changes it with the same functions as the buttons. */
export const shell = {
  vp1: null as Viewport | null,
  vp2: null as Viewport | null,
  vp1State: 'docked' as 'docked' | 'hidden' | 'closed' | 'floating',
  tab: 'inspector' as 'vp2' | 'inspector',
  full: false,
  layout() {
    const s = shell.vp1State;
    const leftHidden = s === 'hidden' || s === 'closed';
    $('left').hidden = leftHidden;
    $('right').classList.toggle('fill', leftHidden);
    $('split').hidden = leftHidden;
    $('slot1').hidden = s !== 'docked';
    $('floating').hidden = s !== 'floating';
    $('slot2').hidden = shell.tab !== 'vp2';
    $('inspector').hidden = shell.tab !== 'inspector';
    for (const tab of document.querySelectorAll<HTMLElement>('.tab')) {
      tab.classList.toggle('on', tab.dataset.tab === shell.tab);
    }
    document.body.classList.toggle('full', shell.full);
    for (const item of document.querySelectorAll<HTMLElement>('.item')) {
      const on = { vp1: s !== 'closed', vp2: shell.tab === 'vp2', inspector: shell.tab === 'inspector', full: shell.full };
      item.textContent = `${on[item.dataset.item as keyof typeof on] ? '[x]' : '[ ]'} ${item.dataset.label}`;
    }
  },
  hide() {
    shell.vp1State = 'hidden';
    shell.layout();
  },
  restore() {
    shell.vp1State = 'docked';
    shell.layout();
  },
  async close() {
    shell.vp1State = 'closed';
    shell.layout();
    await shell.vp1?.unregister();
    shell.vp1?.el.remove();
    shell.vp1 = null;
  },
  async reopen() {
    if (shell.vp1State === 'closed') {
      shell.vp1 = new Viewport(1, mode);
      $('slot1').append(shell.vp1.el);
      await shell.vp1.register();
    }
    if (shell.vp1State !== 'floating') shell.restore();
  },
  async float() {
    if (shell.vp1State !== 'docked' || !shell.vp1) return;
    shell.vp1State = 'floating';
    shell.layout();
    await shell.vp1.unregister();
    await invoke('float_viewport', { id: 1, mode });
  },
  async docked() {
    if (shell.vp1State !== 'floating' || !shell.vp1) return;
    await shell.vp1.register();
    shell.restore();
  },
  setTab(tab: 'vp2' | 'inspector') {
    shell.tab = tab;
    shell.layout();
  },
  setFull(full: boolean) {
    shell.full = full;
    shell.layout();
  },
  /** Opens the menu below the Views button, or at a point of the page (a context menu). */
  menu(open: boolean, x = 60, y = 28) {
    $('menu').hidden = !open;
    $('menu').style.left = `${Math.min(x, innerWidth - 230)}px`;
    $('menu').style.top = `${Math.min(y, innerHeight - 140)}px`;
  },
  /** Sets the width of the right dock, as the splitter does. */
  setRightWidth(css: number) {
    const width = Math.max(150, Math.min(innerWidth - 200, css));
    $('right').style.flexBasis = `${width}px`;
  },
};

function buildShell() {
  document.body.innerHTML = `
    <div id="app">
      <div id="top" class="bar">
        <span class="logo">Patina</span><button id="views">Views</button><span class="grow"></span>
        <span id="mode"></span>
      </div>
      <div id="main">
        <div id="left" class="dock">
          <div class="head">
            <span class="title">Viewport 1</span>
            <button id="float">Float</button><button id="hide">Hide</button><button id="close">Close</button>
          </div>
          <div id="slot1" class="slot"></div>
          <div id="floating" class="placeholder">Viewport 1 is in its own window.</div>
        </div>
        <div id="split"></div>
        <div id="right" class="dock">
          <div class="head">
            <button class="tab" data-tab="vp2">Viewport 2</button>
            <button class="tab" data-tab="inspector">Inspector</button>
          </div>
          <div id="slot2" class="slot"></div>
          <div id="inspector" class="panel"><label>Name <input id="name" value="Cube grid" /></label></div>
        </div>
      </div>
      <div id="status" class="bar"></div>
      <div id="menu" class="menu" hidden>
        <div class="item" data-item="vp1" data-label="Viewport 1"></div>
        <div class="item" data-item="vp2" data-label="Viewport 2"></div>
        <div class="item" data-item="inspector" data-label="Inspector"></div>
        <div class="item" data-item="full" data-label="Full view"></div>
      </div>
    </div>`;
  $('mode').textContent = `frame path: ${mode}`;
  $('views').onclick = (e) => {
    e.stopPropagation();
    shell.menu($('menu').hidden !== false);
  };
  document.addEventListener('pointerdown', (e) => {
    if (!$('menu').contains(e.target as Node) && e.target !== $('views')) shell.menu(false);
  });
  $('main').addEventListener('contextmenu', (e) => {
    e.preventDefault();
    if ((e.target as HTMLElement).closest('.vp')) shell.menu(true, e.clientX, e.clientY);
  });
  for (const item of document.querySelectorAll<HTMLElement>('.item')) {
    item.onclick = () => {
      shell.menu(false);
      const which = item.dataset.item;
      if (which === 'vp1') shell.reopen();
      else if (which === 'full') shell.setFull(!shell.full);
      else shell.setTab(which as 'vp2' | 'inspector');
    };
  }
  for (const tab of document.querySelectorAll<HTMLElement>('.tab')) {
    tab.onclick = () => shell.setTab(tab.dataset.tab as 'vp2' | 'inspector');
  }
  $('float').onclick = () => shell.float();
  $('hide').onclick = () => shell.hide();
  $('close').onclick = () => shell.close();
  const split = $('split');
  split.addEventListener('pointerdown', (e) => split.setPointerCapture(e.pointerId));
  split.addEventListener('pointermove', (e) => {
    if (split.hasPointerCapture(e.pointerId)) shell.setRightWidth(innerWidth - e.clientX - 2);
  });
}

async function startMain() {
  buildShell();
  shell.vp1 = new Viewport(1, mode);
  shell.vp2 = new Viewport(2, mode);
  $('slot1').append(shell.vp1.el);
  $('slot2').append(shell.vp2.el);
  shell.layout();
  await syncClock();
  await shell.vp1.register();
  await shell.vp2.register();
  await listen<number>('docked', () => shell.docked());
  loop(() => [shell.vp1, shell.vp2]);
  if (bench) {
    await runBench();
  } else {
    setInterval(status, 1000);
  }
}

/** Shows one viewport alone, in a float window. */
async function startFloat(id: number) {
  document.body.innerHTML = `
    <div id="app">
      <div id="top" class="bar"><span class="logo">Patina</span><span class="grow">Viewport ${id}</span>
        <button id="dock">Dock</button></div>
      <div id="main"><div id="slot1" class="slot"></div></div>
    </div>`;
  const vp = new Viewport(id, mode);
  $('slot1').append(vp.el);
  await syncClock();
  await vp.register();
  loop(() => [vp]);
  const dock = () => invoke('dock').catch(reportError);
  $('dock').onclick = dock;
  await listen('dock', dock);
  if (bench) {
    // The bench of the main window cannot see this window, so this window checks itself.
    await new Promise((r) => setTimeout(r, 700));
    const rect = vp.physRect();
    const [fraction] = await capture(`float-${id}`, `${mode}-float`, [rect], seen);
    await emitTo('main', 'float-probe', { fraction, rect, captures });
  }
}

function loop(views: () => (Viewport | null)[]) {
  const frame = () => {
    page.ticks++;
    for (const v of views()) v?.tick();
    requestAnimationFrame(frame);
  };
  requestAnimationFrame(frame);
}

async function status() {
  const s = await invoke<Record<string, number | string>>('read_stats');
  await invoke('scenario_begin');
  const secs = (performance.now() - page.since) / 1000;
  const latency = mode === 'native' ? s.native_latency_ms_p50 : percentile(page.latencyMs, 0.5);
  $('status').textContent =
    `${s.adapter} | render loop ${s.loop_fps} fps | viewport frames ${s.view_fps}/s | ` +
    `page ${Math.round(page.ticks / secs)} fps | input latency p50 ${latency} ms | copied ${s.copy_mb_per_s} MB/s`;
  page.reset();
}

(floatId ? startFloat(floatId) : startMain()).catch((e) => reportError(String(e)));

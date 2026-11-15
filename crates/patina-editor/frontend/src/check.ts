// The self-check of `patina --check <dir>`: it makes a project, edits, runs, saves and opens the
// project again in the real window, then writes the result for the Rust side.

import { api, onEngine, onOutput, outputLines, type EngineEvent, type View } from './api';
import { bufferOf, flushCode, openFile, unsavedFiles } from './code';
import { revealOf, type Reveal } from './CodeTab';
import { flushLayout, savedViewport } from './Dock';
import { layoutText, parseLayout } from './layout';

const BUILD_MS = 150_000;
const RUN_MS = 20_000;
const A = 'a000000000000001';
const B = 'b000000000000002';

/** True if a pixel of the RGBA column passes the test. */
function anyPixel(column: Uint8Array, test: (r: number, g: number, b: number) => boolean): boolean {
  for (let i = 0; i < column.length; i += 4) if (column[i + 3] > 128 && test(column[i], column[i + 1], column[i + 2])) return true;
  return false;
}

const amber = (r: number, _g: number, b: number) => r - b > 80;
const gray = (r: number, g: number, b: number) => Math.abs(r - g) < 12 && Math.abs(r - b) < 20;

/** Reads a column of 16 px of the wire canvas at a screen point, by default the middle of the first wire. */
async function wireColumn(at?: { x: number; y: number }): Promise<{ x: number; y: number; column: Uint8Array }> {
  const canvas = document.querySelector<HTMLCanvasElement>('.wire-layer');
  const path = document.querySelector<SVGPathElement>('.wire-hit');
  const pane = document.querySelector('.react-flow__pane');
  if (!canvas || !path || !pane || document.querySelector('.svg-wires')) throw new Error('the WebGL wire layer is not active');
  const gl = canvas.getContext('webgl2')!;
  for (let tries = 0; tries < 5; tries++) {
    const box = path.getBoundingClientRect();
    const x = at?.x ?? box.left + box.width / 2;
    const y = at?.y ?? box.top + box.height / 2;
    // A small zoom at the middle of the wire makes the layer draw in the next frame. This callback
    // runs after that draw and before the frame shows, so the drawing buffer still has the wires.
    pane.dispatchEvent(new WheelEvent('wheel', { deltaY: -1, clientX: x, clientY: y, bubbles: true, cancelable: true }));
    const column = await new Promise<Uint8Array>((resolve) =>
      requestAnimationFrame(() => {
        const rect = canvas.getBoundingClientRect();
        const scale = canvas.width / rect.width;
        const h = Math.round(16 * scale);
        const top = Math.round((y - 8 - rect.top) * scale);
        const out = new Uint8Array(h * 4);
        gl.readPixels(Math.round((x - rect.left) * scale), canvas.height - top - h, 1, h, gl.RGBA, gl.UNSIGNED_BYTE, out);
        resolve(out);
      }),
    );
    if (at || anyPixel(column, () => true)) return { x, y, column };
  }
  throw new Error('the wire layer drew nothing at the middle of the wire');
}

export async function runCheck(dir: string, show: (v: View) => void, pick: (at: Reveal) => void, edited: (v: View) => void) {
  const steps: string[] = [];
  const events: EngineEvent[] = [];
  const output: string[] = [];
  let wake = () => {};
  let outputFrom = 0;
  const stopEngine = await onEngine((e) => {
    events.push(e);
    wake();
  });
  const stopOutput = await onOutput((o) => {
    output.push(...outputLines(o.text));
    wake();
  });

  async function until(what: string, ms: number, test: () => boolean) {
    const end = Date.now() + ms;
    while (!test()) {
      const left = end - Date.now();
      if (left <= 0) throw new Error(`no result after ${ms} ms: ${what}`);
      await new Promise<void>((r) => {
        wake = r;
        setTimeout(r, Math.min(left, 250));
      });
    }
  }

  /** Waits for the result of the engine after `from`, and for an output line. */
  async function result(v: View, from: number, line: string, ms: number) {
    let ran: Extract<EngineEvent, { state: 'ran' }> | undefined;
    await until(`a run that prints ${line}`, ms, () => {
      for (const e of events.slice(from)) {
        if (e.serial !== v.serial) continue;
        if (e.state === 'failed') throw new Error(`the build failed: ${e.text}`);
        if (e.state === 'ran') {
          if (!e.ok) throw new Error(`the run failed: ${JSON.stringify(e.reports)}`);
          ran = e;
        }
      }
      return ran !== undefined && output.slice(outputFrom).includes(line);
    });
    return ran!;
  }

  const step = async (name: string, job: () => Promise<View>) => {
    const v = await job();
    show(v);
    steps.push(name);
    return v;
  };
  const mark = () => {
    outputFrom = output.length;
    return events.length;
  };

  try {
    let v = await step('new project', () => api.newProject(dir, 'check'));
    if (!v.trusted || v.live || (v.graph.node ?? []).length !== 0) throw new Error('a new project is trusted, paused and empty');
    v = await step('add nodes', () =>
      api.edit({
        kind: 'Group',
        edits: [
          { kind: 'AddNode', id: A, folder: 'nodes/number', pos: [0, 0] },
          { kind: 'AddNode', id: B, folder: 'nodes/show', pos: [300, 0] },
          { kind: 'Connect', from: `${A}.out`, to: `${B}.value` },
        ],
      }),
    );
    v = await step('set a parameter', () => api.edit({ kind: 'SetParam', id: A, key: 'value', text: '2.5' }));
    if (v.problems.length > 0) throw new Error(`problems: ${v.problems.join('; ')}`);

    let from = mark();
    v = await step('run', () => api.run());
    const first = await result(v, from, '2.5', BUILD_MS);
    if (first.builtMs === null) throw new Error('the first run builds the graph');

    from = mark();
    v = await step('live edit', () => api.edit({ kind: 'SetParam', id: A, key: 'value', text: '7' }));
    const live = await result(v, from, '7', RUN_MS);
    if (live.builtMs !== null) throw new Error('a change of a parameter does not build again');

    from = mark();
    v = await step('undo', () => api.undo());
    await result(v, from, '2.5', RUN_MS);

    // A compile error keeps the last build that compiled running (15.10). The code, the node,
    // Logs and the status bar show it (15.5).
    const lib = 'nodes/number/src/lib.rs';
    const original = await api.readCode(lib);
    from = mark();
    v = await step('break the code', () => api.edit({ kind: 'SetCode', path: lib, text: `${original}
fn broken() -> u32 {
    "x"
}
` }));
    const broken = await result(v, from, '2.5', BUILD_MS);
    const error = broken.errors.find((e) => e.file === lib);
    if (!error) throw new Error(`no compile error in ${lib}: ${JSON.stringify(broken.errors)}`);
    pick(revealOf(error, Date.now())!);
    await until('the file in the Code tab', RUN_MS, () => bufferOf(lib) !== undefined && document.querySelector('.monaco-editor') !== null);
    await until('the error mark in the gutter', RUN_MS, () => document.querySelector('.gutter-error') !== null);
    await until('the error on the node and in the status bar', RUN_MS, () =>
      document.querySelector('.patina-node.compile-error') !== null && document.querySelector('.status-link') !== null,
    );
    steps.push('compile error shown');
    await new Promise((r) => setTimeout(r, 300));
    await api.capture('code-error');

    bufferOf(lib)!.setValue(original);
    await until('the unsaved dot on the file tab', RUN_MS, () => document.querySelector('.code-tab .files .dot') !== null);
    from = mark();
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 's', ctrlKey: true, bubbles: true }));
    const fixed = await result(v, from, '2.5', BUILD_MS);
    if (fixed.builtMs === null || fixed.errors.length > 0) throw new Error(`the save did not build the fixed code: ${JSON.stringify(fixed.errors)}`);
    if ((await api.readCode(lib)) !== original) throw new Error('the save did not write the code');
    await until('no compile error and no unsaved code', RUN_MS, () =>
      document.querySelector('.patina-node.compile-error') === null && unsavedFiles().length === 0,
    );
    steps.push('fix with Ctrl+S');

    v = await step('save', () => api.save());
    if (v.dirty) throw new Error('the project is still dirty after a save');
    const saved = JSON.stringify(v.graph);
    await step('pause', () => api.pause());

    // The dock (15.2): the menu of a tab moves it, the keys move a splitter, and the layout and the
    // pan and zoom of the graph come back when the project opens again.
    const tab = (name: string) => [...document.querySelectorAll<HTMLElement>('[role=tab]')].find((t) => t.textContent === name);
    const menuItem = (label: string) => [...document.querySelectorAll<HTMLButtonElement>('.menu button')].find((b) => b.textContent === label);
    tab('Logs')!.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true }));
    await until('the menu of the tab', RUN_MS, () => menuItem('Move to the right area') !== undefined);
    menuItem('Move to the right area')!.click();
    await until('Logs in the right area', RUN_MS, () => !!tab('Logs')?.closest('.area.right'));
    const splitter = () => document.querySelector<HTMLElement>('[aria-label="Size of the left area"]')!;
    splitter().dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', shiftKey: true, bubbles: true }));
    await until('a wider left area', RUN_MS, () => splitter().getAttribute('aria-valuenow') === '324');
    const pane = document.querySelector('.react-flow__pane')!.getBoundingClientRect();
    const was = savedViewport('main');
    const at = { clientX: pane.left + pane.width / 2, clientY: pane.top + pane.height / 2 };
    document.querySelector('.react-flow__pane')!.dispatchEvent(new WheelEvent('wheel', { deltaY: -100, ...at, bubbles: true, cancelable: true }));
    await until('the saved pan and zoom', RUN_MS, () => savedViewport('main') !== was);
    const transform = () => document.querySelector<HTMLElement>('.react-flow__viewport')?.style.transform;
    const zoomed = transform();
    await flushLayout();
    const file = parseLayout(await api.layout(v.id));
    if (!file.areas.right.tabs.includes('logs') || file.areas.left.size !== 324 || !file.viewports.main) {
      throw new Error(`the saved layout is not the layout of the window: ${layoutText(file)}`);
    }
    steps.push('dock layout saved');

    await api.closeProject();
    const start = await api.startInfo();
    if (start.recent[0]?.path !== v.root) throw new Error(`the recent list starts with ${start.recent[0]?.path}`);
    steps.push('close');

    v = await step('open again', () => api.openProject(v.root));
    if (JSON.stringify(v.graph) !== saved) throw new Error('the graph after open is not the saved graph');
    if (!v.trusted || v.live) throw new Error('an open project is trusted and paused');
    await until('Logs in the right area after open', RUN_MS, () => !!tab('Logs')?.closest('.area.right'));
    if (transform() !== zoomed) throw new Error(`the pan and zoom after open are ${transform()}, not ${zoomed}`);
    const views = [...document.querySelectorAll<HTMLButtonElement>('.topbar button')].find((b) => b.textContent === 'Views')!;
    views.click();
    await until('the Views menu', RUN_MS, () => menuItem('Reset layout') !== undefined);
    menuItem('Reset layout')!.click();
    await until('Logs in the bottom area after Reset layout', RUN_MS, () => !!tab('Logs')?.closest('.area.bottom'));
    if (transform() !== zoomed) throw new Error('Reset layout changed the pan and zoom');
    steps.push('dock layout restored and reset');

    from = mark();
    v = await step('run again', () => api.run());
    await result(v, from, '2.5', BUILD_MS);
    await until('two ready nodes on the canvas', RUN_MS, () => document.querySelectorAll('.patina-node.ready').length === 2);
    steps.push('ready on the canvas');
    const badge = () => document.querySelector('.wire-value');
    await until('the live value on the wire', RUN_MS, () => badge()?.textContent === '2.5');
    if (badge()!.classList.contains('stale')) throw new Error('the value of the last run is stale');
    steps.push('live value on the wire');

    // The wire draws in WebGL (15.1). Its hit area is wider than the line, and a click selects it (15.9).
    const wire = await wireColumn();
    if (!anyPixel(wire.column, gray) || anyPixel(wire.column, amber)) throw new Error('the wire is not a neutral line');
    const hit = document.elementFromPoint(wire.x, wire.y + 4);
    if (!hit?.closest('.react-flow__edge')) throw new Error(`no wire 4 px from the line: ${hit?.getAttribute('class')}`);
    hit.dispatchEvent(new MouseEvent('click', { bubbles: true, clientX: wire.x, clientY: wire.y + 4 }));
    await until('the selected wire', RUN_MS, () => document.querySelector('.react-flow__edge.selected') !== null);
    if (!anyPixel((await wireColumn()).column, amber)) throw new Error('the selected wire has no accent mark');
    steps.push('wire drawn and selected');
    // The Inspector shows the full text of the selected wire (15.7), from the engine.
    await until('the value in the Inspector', RUN_MS, () => document.querySelector('.value-text')?.textContent === '2.5');
    if ((await api.value(`${A}.out`)) !== '2.5') throw new Error('the engine has no full text of the value');
    steps.push('value in the inspector');

    // Route points (15.9): a double-click adds one, a drag moves it, and Delete on its handle
    // removes the point but keeps the wire. Undo and redo go through the engine.
    const hitPath = document.querySelector<SVGPathElement>('.wire-hit')!;
    const line = hitPath.getBoundingClientRect();
    hitPath.dispatchEvent(new MouseEvent('dblclick', { bubbles: true, clientX: line.left + line.width / 2, clientY: line.top + line.height / 2 }));
    await until('a route point', RUN_MS, () => document.querySelectorAll('.route-point').length === 1);
    steps.push('route point added');
    const handle = () => document.querySelector<HTMLElement>('.route-point')!;
    const center = (b = handle().getBoundingClientRect()) => [b.left + b.width / 2, b.top + b.height / 2];
    const [cx, cy] = center();
    for (const [type, y] of [['pointerdown', cy], ['pointermove', cy + 60], ['pointerup', cy + 60]] as const) {
      handle().dispatchEvent(new PointerEvent(type, { bubbles: true, button: 0, pointerId: 1, isPrimary: true, clientX: cx, clientY: y }));
    }
    await until('the route point under the pointer after the edit', RUN_MS, () =>
      Math.abs(center()[1] - (cy + 60)) < 2 && !handle().classList.contains('dragging'),
    );
    if (anyPixel((await wireColumn({ x: cx, y: cy })).column, () => true)) throw new Error('the wire still draws at the old place of the route point');
    steps.push('route point dragged');
    const added = (await step('undo the drag', () => api.undo())).graph.wire?.[0]?.points;
    const moved = (await step('redo the drag', () => api.redo())).graph.wire?.[0]?.points;
    if (added?.length !== 1 || moved?.length !== 1 || !(moved[0][1] > added[0][1])) throw new Error(`the drag did not move the point: ${JSON.stringify([added, moved])}`);
    handle().focus();
    handle().dispatchEvent(new KeyboardEvent('keydown', { key: 'Delete', bubbles: true, cancelable: true }));
    await until('no route point', RUN_MS, () => document.querySelectorAll('.route-point').length === 0);
    v = await step('undo the removal', () => api.undo());
    if (v.graph.wire?.length !== 1 || v.graph.wire[0].points?.length !== 1) throw new Error('Delete on a route point changed more than the point');
    // While the graph is paused, an edit makes the value of the last run stale (15.3, 15.9).
    await step('pause', () => api.pause());
    v = await step('edit while paused', () => api.edit({ kind: 'SetParam', id: A, key: 'value', text: '3' }));
    edited(v);
    await until('the stale value on the wire', RUN_MS, () => badge()?.classList.contains('stale') === true && badge()?.textContent === '2.5');
    v = await step('undo the edit', () => api.undo());
    v = await step('save the route', () => api.save());
    if (v.dirty) throw new Error('the project is still dirty after a save');

    // The recovery file (15.10): an open after a stop of Patina restores the unsaved graph and code.
    const [bx, by] = v.graph.node!.find((n) => n.id === B)!.pos!;
    v = await step('change without a save', () => api.edit({ kind: 'MoveNode', id: B, pos: [bx, by + 40] }));
    const changedGraph = JSON.stringify(v.graph);
    const changedCode = `${original}// recovered\n`;
    (await openFile(lib)).setValue(changedCode);
    await flushCode();
    v = await step('open after a stop', () => api.openProject(v.root));
    if (!v.recovery || v.dirty) throw new Error('the open did not offer the recovery file');
    const restore = () => [...document.querySelectorAll<HTMLButtonElement>('.dialog button')].find((b) => b.textContent === 'Restore');
    await until('the Restore button', RUN_MS, () => restore() !== undefined);
    restore()!.click();
    const dirtyDot = () => document.querySelector('.topbar [title="Unsaved changes"]');
    await until('the restored code and graph', RUN_MS, () => bufferOf(lib)?.getValue() === changedCode && !!dirtyDot() && !restore());
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 's', ctrlKey: true, bubbles: true }));
    await until('the save of the restored changes', RUN_MS, () => !dirtyDot());
    if ((await api.readCode(lib)) !== changedCode) throw new Error('the save did not write the restored code');
    v = await step('open after the save', () => api.openProject(v.root));
    if (v.recovery || JSON.stringify(v.graph) !== changedGraph) throw new Error('the save did not keep the restored graph or remove the recovery file');
    steps.push('recovery file restored');

    await new Promise((r) => setTimeout(r, 300));
    await api.capture('check');
    steps.push('screenshot');
    await api.checkDone({ ok: true, steps });
  } catch (e) {
    await api.checkDone({ ok: false, steps, error: String(e), output: output.slice(-20) });
  } finally {
    stopEngine();
    stopOutput();
  }
}

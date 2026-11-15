// The self-check of `patina --check <dir>`: it makes a project, edits, runs, saves and opens the
// project again in the real window, then writes the result for the Rust side.

import { api, onEngine, onOutput, outputLines, type EngineEvent, type View } from './api';
import { bufferOf, unsavedFiles } from './code';
import { revealOf, type Reveal } from './CodeTab';

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

/** Reads a column of 16 px of the wire canvas across the middle of the first wire. */
async function wireColumn(): Promise<{ x: number; y: number; column: Uint8Array }> {
  const canvas = document.querySelector<HTMLCanvasElement>('.wire-layer');
  const path = document.querySelector<SVGPathElement>('.wire-hit');
  const pane = document.querySelector('.react-flow__pane');
  if (!canvas || !path || !pane || document.querySelector('.svg-wires')) throw new Error('the WebGL wire layer is not active');
  const gl = canvas.getContext('webgl2')!;
  for (let tries = 0; tries < 5; tries++) {
    const box = path.getBoundingClientRect();
    const x = box.left + box.width / 2;
    const y = box.top + box.height / 2;
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
    if (anyPixel(column, () => true)) return { x, y, column };
  }
  throw new Error('the wire layer drew nothing at the middle of the wire');
}

export async function runCheck(dir: string, show: (v: View) => void, pick: (at: Reveal) => void) {
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
    if (v.dirty) throw new Error('the project is not dirty after a save');
    const saved = JSON.stringify(v.graph);
    await step('pause', () => api.pause());

    await api.closeProject();
    const start = await api.startInfo();
    if (start.recent[0]?.path !== v.root) throw new Error(`the recent list starts with ${start.recent[0]?.path}`);
    steps.push('close');

    v = await step('open again', () => api.openProject(v.root));
    if (JSON.stringify(v.graph) !== saved) throw new Error('the graph after open is not the saved graph');
    if (!v.trusted || v.live) throw new Error('an open project is trusted and paused');

    from = mark();
    v = await step('run again', () => api.run());
    await result(v, from, '2.5', BUILD_MS);
    await until('two ready nodes on the canvas', RUN_MS, () => document.querySelectorAll('.patina-node.ready').length === 2);
    steps.push('ready on the canvas');

    // The wire draws in WebGL (15.1). Its hit area is wider than the line, and a click selects it (15.9).
    const wire = await wireColumn();
    if (!anyPixel(wire.column, gray) || anyPixel(wire.column, amber)) throw new Error('the wire is not a neutral line');
    const hit = document.elementFromPoint(wire.x, wire.y + 4);
    if (!hit?.closest('.react-flow__edge')) throw new Error(`no wire 4 px from the line: ${hit?.getAttribute('class')}`);
    hit.dispatchEvent(new MouseEvent('click', { bubbles: true, clientX: wire.x, clientY: wire.y + 4 }));
    await until('the selected wire', RUN_MS, () => document.querySelector('.react-flow__edge.selected') !== null);
    if (!anyPixel((await wireColumn()).column, amber)) throw new Error('the selected wire has no accent mark');
    steps.push('wire drawn and selected');

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

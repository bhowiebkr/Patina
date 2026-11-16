// The self-check of `patina --check <dir>`: it makes a project, edits, runs, saves and opens the
// project again in the real window, then writes the result for the Rust side.

import { api, onEngine, onOutput, outputLines, type EngineEvent, type View } from './api';
import { bufferOf, flushCode, openFile, unsavedFiles } from './code';
import { revealOf, type Reveal } from './CodeTab';
import { emit, listen } from '@tauri-apps/api/event';
import { changeLayout, flushLayout, savedViewport } from './Dock';
import { floatPanel, layoutText, parseLayout } from './layout';

const BUILD_MS = 150_000;
const RUN_MS = 20_000;
const A = 'a000000000000001';
const B = 'b000000000000002';

/** A stateful number node with a GUI (4.2, 4.3): its button adds 1 to the output. It also shows each value widget. */
// A view (15.8) that shows a parameter of node A, and sets it with a button.
const VIEW_CODE = `use patina_sdk::{Graph, Ui, view};

#[view]
pub struct Panel;

impl Panel {
    pub fn init() -> Self {
        Panel
    }

    pub fn ui(&mut self, ui: &mut Ui, graph: &mut Graph) {
        ui.label(&format!("value {}", graph.param("${A}", "value").unwrap_or_default()));
        if ui.button("Set").clicked() {
            graph.set_param("${A}", "value", 5.0);
        }
    }
}
`;

const GUI_NODE = `use patina_sdk::node;

#[node]
pub struct Number {
    added: f64,
    row: Option<usize>,
    tint: [u8; 3],
    points: Vec<[f32; 2]>,
}

impl Number {
    pub fn init() -> Self {
        Number { added: 0.0, row: None, tint: [223, 166, 75], points: vec![[0.0, 0.2], [0.5, 0.8], [1.0, 0.4]] }
    }

    pub fn process(&mut self, value: f64) -> f64 {
        value + self.added
    }

    pub fn ui(&mut self, ui: &mut patina_sdk::Ui) {
        if ui.button("Add 1").clicked() {
            self.added += 1.0;
        }
        ui.color("Tint", &mut self.tint);
        let wave: Vec<f32> = (0..400).map(|i| (i as f32 * 0.2).sin() * (1.0 - i as f32 / 400.0)).collect();
        ui.plot("Wave plot", &wave[..100], -1.0..=1.0);
        ui.waveform("Waveform", &wave, Some(0.3));
        let added = self.added.to_string();
        ui.table("Values", &["Name", "Value"], &["added", added.as_str(), "points", "3"], &mut self.row);
        ui.curve("Curve", &mut self.points);
        let pixels: Vec<u8> = (0..256_u32).flat_map(|i| [(i % 32 * 8) as u8, 96, (255 - i % 32 * 8) as u8, 255]).collect();
        ui.image("Image", [32, 8], &pixels);
    }
}
`;

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
    // The settings screen (15.10): the start screen opens it. It shows the Rust version of the
    // toolchain and the list of trusted projects. Back returns to the start screen.
    const button = (text: string) => [...document.querySelectorAll<HTMLButtonElement>('button')].find((b) => b.textContent === text);
    const section = (name: string) => document.querySelector(`section[aria-label="${name}"]`)?.textContent ?? '';
    await until('the Settings button', RUN_MS, () => !!button('Settings'));
    button('Settings')!.click();
    const trustedList = () => section('Trusted projects').endsWith('None') || !!document.querySelector('.settings-row');
    await until('the toolchain and the trusted projects', RUN_MS, () => /Rust\d/.test(section('Toolchain')) && trustedList());
    await api.capture('settings');
    button('Back')!.click();
    // The check does not click Browse: the folder dialog of the platform would wait for a person.
    const browse = () => [...document.querySelectorAll('button')].filter((b) => b.textContent === 'Browse').length;
    await until('the start screen with two Browse buttons', RUN_MS, () => !!button('Settings') && browse() === 2);
    await api.capture('start');
    steps.push('settings screen');

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
    // A second save during a build drops the first build, so only the newest errors show (7.2).
    await api.edit({ kind: 'SetCode', path: lib, text: `${original}
fn first_error() -> u32 {
    1.5
}
` });
    v = await step('break the code', () => api.edit({ kind: 'SetCode', path: lib, text: `${original}
fn broken() -> u32 {
    "x"
}
` }));
    const broken = await result(v, from, '2.5', BUILD_MS);
    const old = events.slice(from).filter((e) => e.state === 'ran' && e.errors.some((d) => d.rendered.includes('first_error')));
    if (old.length > 0) throw new Error('a run showed the errors of an older save');
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

    // The fixed code also panics at 13 (15.3), and waits while the value is 99 until the user
    // clicks Stop (15.2).
    const stoppable = original.replace(/    value\r?\n\}/, `    assert!(value != 13.0, "the check panics at 13");
    while value == 99.0 && !patina_sdk::stop_requested() {
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    value
}`);
    if (stoppable === original) throw new Error(`the Stop check cannot change the code of ${lib}`);
    bufferOf(lib)!.setValue(stoppable);
    await until('the unsaved dot on the file tab', RUN_MS, () => document.querySelector('.code-tab .files .dot') !== null);
    from = mark();
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 's', ctrlKey: true, bubbles: true }));
    const fixed = await result(v, from, '2.5', BUILD_MS);
    if (fixed.builtMs === null || fixed.errors.length > 0) throw new Error(`the save did not build the fixed code: ${JSON.stringify(fixed.errors)}`);
    if ((await api.readCode(lib)) !== stoppable) throw new Error('the save did not write the code');
    // A save of a Rust file of a node puts the node in the slot (7.1).
    if (fixed.slot !== 'number') throw new Error(`the save did not put the node in the slot: ${fixed.slot}`);
    await until('the slot icon on the node', RUN_MS, () => document.querySelector('.patina-node [title="In the slot"]') !== null);
    // The Layers panel shows the node in the slot with the time of its build (7.3).
    const leftTab = (name: string) => [...document.querySelectorAll<HTMLElement>('.area.left [role=tab]')].find((t) => t.textContent === name)!;
    leftTab('Layers').click();
    await until('the slot row of the Layers panel', RUN_MS, () => /Slotnumber - Edit buildBuilt in/.test(document.querySelector('.layers')?.textContent ?? ''));
    leftTab('Project').click();
    await until('no compile error and no unsaved code', RUN_MS, () =>
      document.querySelector('.patina-node.compile-error') === null && unsavedFiles().length === 0,
    );
    steps.push('fix with Ctrl+S');

    const statusText = () => document.querySelector('.statusbar')?.textContent ?? '';
    const nodeClass = (id: string) => document.querySelector(`.react-flow__node[data-id="${id}"] .patina-node`)?.classList;
    const endOf = async (v: View, from: number) => {
      let ended: EngineEvent | undefined;
      await until('the end of the run', RUN_MS, () => (ended = events.slice(from).find((e) => e.serial === v.serial && e.state !== 'started')) !== undefined);
      return ended!;
    };

    // A node after a failed node is skipped (15.3).
    from = mark();
    v = await step('a node that panics', () => api.edit({ kind: 'SetParam', id: A, key: 'value', text: '13' }));
    const failed = await endOf(v, from);
    if (failed.state !== 'ran' || failed.ok || failed.skipped.join() !== B) {
      throw new Error(`node ${B} is not skipped after a panic: ${JSON.stringify(failed)}`);
    }
    await until('a failed node and a skipped node', RUN_MS, () =>
      statusText().includes('1 skipped node') && nodeClass(A)?.contains('run-error') === true && nodeClass(B)?.contains('skipped') === true,
    );
    from = mark();
    v = await step('undo the panic', () => api.undo());
    await result(v, from, '2.5', RUN_MS);
    steps.push('skip the node after a panic');

    // Stop (15.2): the node waits in its loop until Stop sets the flag. Then the show node does not start.
    from = mark();
    v = await step('a node that waits', () => api.edit({ kind: 'SetParam', id: A, key: 'value', text: '99' }));
    const waitEnd = Date.now() + RUN_MS;
    while ((await api.running()) !== A) {
      if (Date.now() > waitEnd) throw new Error(`no run of node ${A} after ${RUN_MS} ms`);
      await new Promise((r) => setTimeout(r, 20));
    }
    const topButton = (label: string) => [...document.querySelectorAll<HTMLButtonElement>('.topbar button')].find((b) => b.textContent === label);
    await until('the Stop button', RUN_MS, () => topButton('Stop')?.disabled === false);
    topButton('Stop')!.click();
    const ended = await endOf(v, from);
    if (ended.state !== 'ran' || !ended.ok || ended.stopped.join() !== B || output.slice(outputFrom).includes('99')) {
      throw new Error(`Stop did not keep node ${B} from its start: ${JSON.stringify(ended)}`);
    }
    await until('a paused graph with one stale node and no run', RUN_MS, () =>
      topButton('Stop')?.disabled === true &&
      topButton('Run')?.disabled === false &&
      statusText().includes('1 stale node') &&
      nodeClass(B)?.contains('stale') === true &&
      nodeClass(A)?.contains('ready') === true,
    );
    if ((await api.running()) !== null) throw new Error('a node runs after the end of the run');
    v = await step('undo the wait', () => api.undo());
    if (v.live) throw new Error('the graph is live after Stop');
    steps.push('stop a node that waits');

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
    const timeOf = (id: string) => document.querySelector(`.react-flow__node[data-id="${id}"] .patina-node-time`)?.textContent ?? '';
    await until('the run time on each node', RUN_MS, () => [A, B].every((id) => /^Run \d+(\.\d+)? (us|ms|s)$/.test(timeOf(id))));
    steps.push(`run time on the nodes: ${timeOf(A)}, ${timeOf(B)}`);
    const badge = () => document.querySelector('.wire-value');
    await until('the live value on the wire', RUN_MS, () => badge()?.textContent === '2.5');
    if (badge()!.classList.contains('stale')) throw new Error('the value of the last run is stale');
    const label = document.querySelector('.wire-type')?.textContent;
    if (label !== 'f64') throw new Error(`the wire type label is ${label}, not f64`);
    steps.push('live value and type label on the wire');

    // Below a zoom of 0.5, the nodes have no row text and the wires have no badge (15.1).
    const scale = () => Number(/scale\(([^)]+)\)/.exec(transform() ?? '')?.[1]);
    const nodeElements = () => document.querySelectorAll('.react-flow__node *').length;
    const wheel = (deltaY: number) =>
      document.querySelector('.react-flow__pane')!.dispatchEvent(new WheelEvent('wheel', { deltaY, ...at, bubbles: true, cancelable: true }));
    const near = nodeElements();
    let outs = 0;
    while (scale() >= 0.5) {
      const z = scale();
      wheel(200);
      outs++;
      await until('a smaller zoom', RUN_MS, () => scale() < z);
    }
    await until('the simple node form', RUN_MS, () =>
      document.querySelectorAll('.patina-row').length === 0 && !document.querySelector('.wire-value') && !document.querySelector('.wire-type'));
    if (document.querySelectorAll('.patina-node .title').length !== 2) throw new Error('a node at a far zoom has no title');
    steps.push(`simple nodes at a far zoom: ${near} node elements near, ${nodeElements()} far`);
    for (; outs > 0; outs--) {
      const z = scale();
      wheel(-200);
      await until('a larger zoom', RUN_MS, () => scale() > z);
    }
    await until('the badge after the far zoom', RUN_MS, () => badge()?.textContent === '2.5' && document.querySelectorAll('.patina-row').length > 0);

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
    // The automatic route goes through the point, and before the drag no wire was 60 px below the line.
    if (!anyPixel((await wireColumn({ x: cx, y: cy + 60 })).column, () => true)) throw new Error('the wire does not draw at the new place of the route point');
    steps.push('route point dragged');
    const added = (await step('undo the drag', () => api.undo())).graph.wire?.[0]?.points;
    const moved = (await step('redo the drag', () => api.redo())).graph.wire?.[0]?.points;
    if (added?.length !== 1 || moved?.length !== 1 || !(moved[0][1] > added[0][1])) throw new Error(`the drag did not move the point: ${JSON.stringify([added, moved])}`);
    handle().focus();
    handle().dispatchEvent(new KeyboardEvent('keydown', { key: 'Delete', bubbles: true, cancelable: true }));
    await until('no route point', RUN_MS, () => document.querySelectorAll('.route-point').length === 0);
    v = await step('undo the removal', () => api.undo());
    if (v.graph.wire?.length !== 1 || v.graph.wire[0].points?.length !== 1) throw new Error('Delete on a route point changed more than the point');
    // The look of the wire (15.9): the Route field of the Inspector sets the style, and Reset route
    // removes the route points. Both are edits with undo.
    const field = (name: string) =>
      [...document.querySelectorAll('.param')].find((p) => p.querySelector('.label')?.textContent === name)?.querySelector('select');
    const rounded = () => (document.querySelector('.wire-hit')?.getAttribute('d')?.match(/Q/g) ?? []).length;
    await until('the Route field and the corners of the automatic route', RUN_MS, () => !!field('Route') && rounded() >= 2);
    field('Route')!.value = 'curved';
    field('Route')!.dispatchEvent(new Event('change', { bubbles: true }));
    await until('the curved wire', RUN_MS, () => rounded() === 0 && !!document.querySelector('.wire-hit')?.getAttribute('d')?.includes('C'));
    [...document.querySelectorAll<HTMLButtonElement>('.panel button')].find((b) => b.textContent === 'Reset route')!.click();
    await until('no route point after Reset route', RUN_MS, () => document.querySelectorAll('.route-point').length === 0);
    v = await step('undo Reset route', () => api.undo());
    if (v.graph.wire?.[0]?.style !== 'curved' || v.graph.wire[0].points?.length !== 1) throw new Error(`the route style or Reset route failed: ${JSON.stringify(v.graph.wire)}`);
    v = await step('undo the route style', () => api.undo());
    if (v.graph.wire?.[0]?.style !== undefined) throw new Error('undo did not give the graph default route');
    // While the graph is paused, an edit makes the value of the last run stale (15.3, 15.9).
    await step('pause', () => api.pause());
    v = await step('edit while paused', () => api.edit({ kind: 'SetParam', id: A, key: 'value', text: '3' }));
    edited(v);
    await until('the stale value on the wire', RUN_MS, () => badge()?.classList.contains('stale') === true && badge()?.textContent === '2.5');
    v = await step('undo the edit', () => api.undo());

    // The Add node palette (15.2): Shift+A opens it and Esc closes it. A double-click on an empty
    // area opens it, and Enter adds the first match at the point of the double-click.
    const search = () => document.querySelector<HTMLInputElement>('.palette input');
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'A', shiftKey: true, bubbles: true }));
    await until('the palette after Shift+A', RUN_MS, () => !!search());
    search()!.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    await until('no palette after Esc', RUN_MS, () => !search());
    document.querySelector('.react-flow__pane')!.dispatchEvent(new MouseEvent('dblclick', { bubbles: true, ...at }));
    await until('the palette after a double-click', RUN_MS, () => !!search());
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!.call(search(), 'SHO');
    search()!.dispatchEvent(new Event('input', { bubbles: true }));
    const options = () => [...document.querySelectorAll('.palette [role=option]')].map((o) => o.textContent);
    await until('one match for SHO', RUN_MS, () => options().join() === 'show');
    search()!.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    const newNode = () => [...document.querySelectorAll('.react-flow__node')].find((n) => ![A, B].includes(n.getAttribute('data-id')!));
    await until('the added node', RUN_MS, () => !search() && newNode()?.querySelector('.title')?.textContent === 'show');
    const box = newNode()!.getBoundingClientRect();
    if (Math.abs(box.left - at.clientX) > 4 || Math.abs(box.top - at.clientY) > 4) throw new Error('the added node is not at the point of the double-click');
    v = await step('undo the added node', () => api.undo());
    if (v.graph.node?.length !== 2) throw new Error('undo did not remove the added node');
    steps.push('add a node with the palette');

    // A drag from a pin into an empty area opens the palette with only the nodes that fit the pin
    // (15.2). The new node and its wire are one undo step.
    const pinEl = document.querySelector(`.react-flow__node[data-id="${B}"] .react-flow__handle[data-handleid="value"]`)!;
    const pinBox = pinEl.getBoundingClientRect();
    pinEl.dispatchEvent(new MouseEvent('mousedown', { bubbles: true, clientX: pinBox.left + pinBox.width / 2, clientY: pinBox.top + pinBox.height / 2 }));
    document.dispatchEvent(new MouseEvent('mousemove', { bubbles: true, ...at }));
    document.querySelector('.react-flow__pane')!.dispatchEvent(new MouseEvent('mouseup', { bubbles: true, ...at }));
    await until('the palette after a drag from a pin', RUN_MS, () => !!search());
    // The show node has no output. The output of the built-in delay is generic, so it fits.
    await until('only the nodes with an f64 output', RUN_MS, () => options().join() === 'number,delaybuilt-in');
    search()!.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    await until('the added node from the pin', RUN_MS, () => !search() && newNode()?.querySelector('.title')?.textContent === 'number');
    const fed = newNode()!.getAttribute('data-id');
    v = await step('undo the node and its wire', () => api.undo());
    if (v.graph.node?.length !== 2 || v.graph.wire?.[0]?.from !== `${A}.out`) throw new Error(`one undo did not remove the node and its wire: ${JSON.stringify(v.graph.wire)}`);
    v = await step('redo the node and its wire', () => api.redo());
    if (v.graph.wire?.length !== 1 || v.graph.wire[0].from !== `${fed}.out` || v.graph.wire[0].to !== `${B}.value`) {
      throw new Error(`the new node does not feed the pin: ${JSON.stringify(v.graph.wire)}`);
    }
    v = await step('undo the node from a pin', () => api.undo());
    steps.push('add a node from a pin');

    // New node (15.2): the + of the Nodes group asks for a name. A used name shows its problem. A
    // new name makes the folder, and the Code tab shows its code.
    const nameField = () => document.querySelector<HTMLInputElement>('[aria-label="Node name"]');
    const enterText = (field: HTMLInputElement, text: string) => {
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!.call(field, text);
      field.dispatchEvent(new Event('input', { bubbles: true }));
      field.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    };
    const enterName = (text: string) => enterText(nameField()!, text);
    document.querySelector<HTMLButtonElement>('[aria-label="New node"]')!.click();
    await until('the name field', RUN_MS, () => !!nameField());
    enterName('Number');
    await until('the problem of a used name', RUN_MS, () => !!document.querySelector('.new-item [role=alert]'));
    enterName('made-node');
    const listed = () => [...document.querySelectorAll('[aria-label=Project] .item')].some((b) => b.textContent === 'made-node');
    const codeTab = () => document.querySelector('[aria-label=Inspector] [aria-selected=true]')?.textContent === 'Code';
    await until('the new node folder and its code', RUN_MS, () => !nameField() && listed() && codeTab());
    steps.push('make a node folder');

    // Add crate (15.4): a bad crate name shows its problem before cargo runs, so no network is used.
    const crateField = () => document.querySelector<HTMLInputElement>('[aria-label="Crate or widget name"]');
    [...document.querySelectorAll<HTMLButtonElement>('.code-bar .btn')].find((b) => b.textContent === 'Add crate')!.click();
    await until('the crate field', RUN_MS, () => !!crateField());
    enterText(crateField()!, '-x');
    const crateProblem = () => document.querySelector('.new-item [role=alert]')?.textContent ?? '';
    await until('the problem of a bad crate name', RUN_MS, () => crateProblem().includes('name@version'));
    crateField()!.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    await until('the closed crate field', RUN_MS, () => !crateField());
    steps.push('refuse a bad crate name');
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
      return frameCount() >= 3 && output.slice(outputFrom).filter((l) => l === '2.5').length >= 3;
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
    v = await step('undo the frame node', () => api.undo());
    steps.push(`frames until Stop: ${count}`);

    // A node GUI (4.3): the node body shows the button of the `ui`. A click in a live graph runs
    // the graph again with the new state, and does not build again.
    v = await step('make a node with a GUI', () => api.edit({ kind: 'SetCode', path: lib, text: GUI_NODE }));
    from = mark();
    v = await step('run the node GUI', () => api.run());
    await result(v, from, '2.5', BUILD_MS);
    const guiButton = () =>
      [...document.querySelectorAll<HTMLButtonElement>(`.react-flow__node[data-id="${A}"] .node-gui button`)].find((b) => b.textContent === 'Add 1');
    await until('the button of the node GUI', RUN_MS, () => guiButton()?.disabled === false);
    from = mark();
    guiButton()!.click();
    const clicked = await result(v, from, '3.5', RUN_MS);
    if (clicked.builtMs !== null) throw new Error('a click in a node GUI built the graph again');
    const gui = `.react-flow__node[data-id="${A}"] .node-gui`;
    for (const kind of ['gui-color', 'gui-chart', 'gui-table', 'gui-curve', 'gui-image']) {
      if (!document.querySelector(`${gui} .${kind}`)) throw new Error(`the node GUI has no ${kind}`);
    }
    const row = () => document.querySelector<HTMLElement>(`${gui} .gui-table tbody tr:nth-child(2)`);
    row()!.click();
    await until('the selected row of the table', RUN_MS, () => row()?.getAttribute('aria-selected') === 'true');
    await api.capture('node-gui');
    tab('Parameters')!.click();
    await until('the large node GUI in the Inspector', RUN_MS, () => document.querySelector('aside[aria-label="Inspector"] .node-gui .gui-image') !== null);
    await api.capture('node-gui-large');
    await step('pause', () => api.pause());
    v = await step('undo the node GUI', () => api.undo());
    steps.push('click a node GUI');

    // A view (15.8): the Views menu opens it in the View panel. Its button writes a parameter
    // through the edit path, so undo removes the write.
    v = await step('make a view', () => api.newView('panel'));
    v = await step('write the view code', () => api.edit({ kind: 'SetCode', path: 'views/panel/src/lib.rs', text: VIEW_CODE }));
    button('Views')!.click();
    const item = () => [...document.querySelectorAll<HTMLElement>('[role="menuitemcheckbox"]')].find((e) => e.textContent?.trim() === 'panel');
    await until('the item of the view in the Views menu', RUN_MS, () => !!item());
    item()!.click();
    const panel = 'aside[aria-label="View"]';
    const viewText = () => {
      const failed = document.querySelector(`${panel} .error-text`)?.textContent;
      if (failed) throw new Error(`the view failed: ${failed}`);
      return document.querySelector(`${panel} .node-gui`)?.textContent ?? '';
    };
    await until('the widgets of the view', BUILD_MS, () => viewText().startsWith('value '));
    if (!viewText().startsWith(`value ${v.graph.node?.find((n) => n.id === A)?.params?.value ?? ''}Set`)) throw new Error(`the view shows ${viewText()}`);
    [...document.querySelectorAll<HTMLButtonElement>(`${panel} .node-gui button`)].find((b) => b.textContent === 'Set')!.click();
    await until('the write of the view', RUN_MS, () => viewText().startsWith('value 5Set'));
    await api.capture('view');
    v = await step('undo the view write', () => api.undo());
    if (v.graph.node?.find((n) => n.id === A)?.params?.value === '5') throw new Error('undo did not remove the write of the view');
    steps.push('a view sets a parameter');
    button('Views')!.click();
    await until('the Views menu again', RUN_MS, () => !!item());
    item()!.click();
    const second = () => [...document.querySelectorAll('[role="tab"]')].find((t) => t.textContent === 'panel 2');
    await until('the tab of a second instance', RUN_MS, () => !!second());
    await until('the widgets of the second instance', BUILD_MS, () => viewText().startsWith('value '));
    steps.push('a second instance of a view');

    // A float window (15.8): the second instance opens in its own window, and Dock brings it back.
    let ready = false;
    const stopReady = await listen<string>('float-ready', (e) => (ready ||= e.payload === 'view2'));
    try {
      changeLayout((l) => floatPanel(l, 'view2'));
      await until('the tab of the float view to go', RUN_MS, () => !second());
      await until('the float window', BUILD_MS, () => ready);
    } finally {
      stopReady();
    }
    await emit('float-dock', 'view2');
    await until('the docked tab of the float view', RUN_MS, () => !!second());
    await until('the widgets of the docked view', BUILD_MS, () => viewText().startsWith('value '));
    steps.push('a float window of a view');

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

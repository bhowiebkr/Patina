// The self-check of `patina --check <dir>`: it makes a project, edits, runs, saves and opens the
// project again in the real window, then writes the result for the Rust side.

import { api, onEngine, onOutput, outputLines, type EngineEvent, type View } from './api';
import { bufferOf, flushCode, monaco, openFile, unsavedFiles } from './code';
import { revealOf, type Reveal } from './CodeTab';
import { emit, listen } from '@tauri-apps/api/event';
import { changeLayout, flushLayout, savedViewport } from './Dock';
import { floatPanel, layoutText, openPanel, parseLayout } from './layout';
import { checkWires } from './check-wires';
import { checkCanvas } from './check-canvas';
import { checkDomains } from './check-domains';
import { checkGuard, checkWidgets } from './check-gui';
import { checkIsolation } from './check-isolation';
import { keepClipInMemory } from './clip';
import { A, B, BUILD_MS, EVENT_NODE, GUI_NODE, RUN_MS, T, VIEW_CODE, enterText, fireButton, mouseWheel } from './check-fixtures';

export async function runCheck(dir: string, show: (v: View) => void, pick: (at: Reveal) => void, edited: (v: View) => void) {
  keepClipInMemory();
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
        if (e.state === 'lost') throw new Error(`the node process ended: ${e.text}`);
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
    // The check does not click Add from Git or Browse libraries: the downloads need the network (6.2).
    await until('the Git URL field of the libraries', RUN_MS, () => !!document.querySelector('input[aria-label="Git URL"]') && !!button('Add from Git') && !!button('Browse libraries'));
    await api.capture('settings');
    button('Back')!.click();
    // The check does not click Browse: the folder dialog of the platform would wait for a person.
    const browse = () => [...document.querySelectorAll('button')].filter((b) => b.textContent === 'Browse').length;
    await until('the start screen with two Browse buttons', RUN_MS, () => !!button('Settings') && browse() === 2);
    await api.capture('start');
    steps.push('settings screen');
    // The About dialog (15.3). The check does not click Check for updates: it needs the network.
    button('About')!.click();
    const about = () => document.querySelector('.dialog.about')?.textContent ?? '';
    await until('the About dialog with Check for updates', RUN_MS, () => !!button('Check for updates') && /Version \d/.test(about()));
    button('Close')!.click();
    await until('the start screen after About', RUN_MS, () => !button('Check for updates'));
    steps.push('about dialog');
    // The content security policy refuses code from text and requests to other hosts. The policy
    // refuses the request before a lookup, and a `.invalid` host never resolves.
    const refused = new Set<string>();
    const onRefuse = (e: SecurityPolicyViolationEvent) => refused.add(e.effectiveDirective);
    document.addEventListener('securitypolicyviolation', onRefuse);
    let evaluated = true;
    try {
      (0, eval)('1');
    } catch {
      evaluated = false;
    }
    const fetched = await fetch('https://patina.invalid/').then(() => true, () => false);
    await until('the reports of the content security policy', RUN_MS, () => refused.has('script-src') && refused.has('connect-src'));
    document.removeEventListener('securitypolicyviolation', onRefuse);
    if (evaluated || fetched) throw new Error(`the content security policy let ${evaluated ? 'eval' : 'a request to another host'} through`);
    steps.push('the content security policy refuses eval and other hosts');

    // A second new project is a library of the check: the palette shows its nodes (6.2).
    const libProject = await step('new library project', () => api.newProject(dir, 'checklib'));
    await api.addLibrary(libProject.root);
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

    // The Code tab keeps the cursor of each file when the file or the Inspector tab changes (15.10).
    const codeEditor = () => monaco.editor.getEditors()[0];
    const lineAt = () => codeEditor()?.getPosition()?.lineNumber;
    const tabOf = (box: string, name: (t: HTMLElement) => boolean) => [...document.querySelectorAll<HTMLElement>(`${box} [role=tab]`)].find(name)!;
    tabOf('.code-tab .files', (t) => t.title.endsWith('/Cargo.toml')).click();
    await until('Cargo.toml in the Code tab', RUN_MS, () => codeEditor()?.getModel()?.uri.path.endsWith('/Cargo.toml') === true);
    tabOf('.code-tab .files', (t) => t.title === lib).click();
    await until('the cursor at the error again', RUN_MS, () => lineAt() === error.start[0]);
    codeEditor()!.setPosition({ lineNumber: 2, column: 1 });
    tabOf('aside[aria-label="Inspector"]', (t) => t.textContent === 'Parameters').click();
    await until('no Code tab', RUN_MS, () => monaco.editor.getEditors().length === 0);
    tabOf('aside[aria-label="Inspector"]', (t) => t.textContent === 'Code').click();
    await until('the cursor on line 2 after the Code tab opens again', RUN_MS, () => lineAt() === 2);
    steps.push('the Code tab keeps the cursor');

    // The fixed code also panics at 13 (15.3), and waits while the value is 99 until the user
    // clicks Stop (15.2). It crashes at 66, overflows its stack at 55, allocates 1 TB at 44, does
    // not stop at 77 and prints without a pause at 88 until Stop (16.1).
    const stoppable = original.replace(/    value\r?\n\}/, `    assert!(value != 13.0, "the check panics at 13");
    if value == 66.0 {
        unsafe { (8usize as *mut u8).write_volatile(1) }
    }
    if value == 55.0 {
        fn deep(n: f64) -> f64 {
            let a = std::hint::black_box([n; 64]);
            if n > 0.0 { deep(n + 1.0) + a[1] } else { a[0] }
        }
        return deep(value);
    }
    if value == 44.0 {
        std::hint::black_box(vec![1u8; 1 << 40]);
    }
    while value == 77.0 {
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    while value == 88.0 && !patina_sdk::stop_requested() {
        println!("the check floods the output");
    }
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

    // A failed build of the slot alone keeps the base, so the next save builds the slot alone (7.1).
    // The error is in the body of the node, so the outline of the code stays the same.
    const slotError = stoppable.replace(/    value\r?\n\}/, '    let broken: u32 = "x";\n    value\n}');
    if (slotError === stoppable) throw new Error(`the slot check cannot change the code of ${lib}`);
    from = mark();
    v = await step('break the slot', () => api.edit({ kind: 'SetCode', path: lib, text: slotError }));
    const slotBroken = await result(v, from, '2.5', BUILD_MS);
    if (!slotBroken.errors.some((e) => e.file === lib)) throw new Error(`no compile error in the slot: ${JSON.stringify(slotBroken.errors)}`);
    from = mark();
    v = await step('fix the slot', () => api.edit({ kind: 'SetCode', path: lib, text: stoppable }));
    const slotFixed = await result(v, from, '2.5', BUILD_MS);
    if (!slotFixed.slotOnly || slotFixed.errors.length > 0) throw new Error(`the save after a slot error built all: ${JSON.stringify(slotFixed.errors)}`);
    await until('no compile error on the node', RUN_MS, () => document.querySelector('.patina-node.compile-error') === null);

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
      topButton('Stop')?.disabled === false &&
      topButton('Run')?.disabled === false &&
      statusText().includes('1 stale node') &&
      nodeClass(B)?.contains('stale') === true &&
      nodeClass(A)?.contains('ready') === true,
    );
    if ((await api.running()) !== null) throw new Error('a node runs after the end of the run');
    v = await step('undo the wait', () => api.undo());
    if (v.live) throw new Error('the graph is live after Stop');
    steps.push('stop a node that waits');
    v = await checkIsolation({ step, until, mark, result, events, steps }, v);

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
    document.querySelector('.react-flow__pane')!.dispatchEvent(mouseWheel(-120, at.clientX, at.clientY));
    await until('the saved pan and zoom', RUN_MS, () => savedViewport('main') !== was);
    // A two-finger touchpad scroll pans on the two axes and keeps the zoom (15.2).
    const prev = savedViewport('main')!;
    document.querySelector('.react-flow__pane')!.dispatchEvent(new WheelEvent('wheel', { deltaX: 30, deltaY: 20, ...at, bubbles: true, cancelable: true }));
    await until('the touchpad pan', RUN_MS, () => {
      const v = savedViewport('main');
      return !!v && v.zoom === prev.zoom && Math.abs(v.x - prev.x + 30) < 0.5 && Math.abs(v.y - prev.y + 20) < 0.5;
    });
    steps.push('touchpad pan');
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

    from = mark();
    v = await step('open again', () => api.openProject(v.root));
    if (JSON.stringify(v.graph) !== saved) throw new Error('the graph after open is not the saved graph');
    if (!v.trusted || v.live) throw new Error('an open project is trusted and paused');
    // An open loads the last build, so the node GUIs show while the graph is paused (15.10).
    const opened = (state: EngineEvent['state']) => events.slice(from).some((e) => e.serial === v.serial && e.state === state);
    await until('the load of the last build', BUILD_MS, () => opened('loaded') || opened('failed'));
    if (!opened('loaded') || opened('ran')) throw new Error('the open did not load the last build, or it ran the graph');
    await api.closeProject();
    from = mark();
    v = await step('open again with no change', () => api.openProject(v.root));
    await until('the second load', BUILD_MS, () => opened('loaded') || opened('failed'));
    if (opened('building') || opened('ran')) throw new Error('an open with no change built or ran the graph');
    await until('Logs in the right area after open', RUN_MS, () => !!tab('Logs')?.closest('.area.right'));
    if (transform() !== zoomed) throw new Error(`the pan and zoom after open are ${transform()}, not ${zoomed}`);
    const views = [...document.querySelectorAll<HTMLButtonElement>('.topbar button')].find((b) => b.textContent === 'Views')!;
    views.click();
    await until('the Views menu', RUN_MS, () => menuItem('Reset layout') !== undefined);
    menuItem('Reset layout')!.click();
    await until('Logs in the bottom area after Reset layout', RUN_MS, () => !!tab('Logs')?.closest('.area.bottom'));
    if (transform() !== zoomed) throw new Error('Reset layout changed the pan and zoom');
    steps.push('dock layout restored and reset');
    // The rail icon of the active left panel folds the left area, and a second click unfolds it.
    const rail = (name: string) => document.querySelector<HTMLButtonElement>(`nav.strip button[aria-label="${name}"]`)!;
    rail('Project').click();
    await until('a folded left area', RUN_MS, () => document.querySelector('.area.left') === null && rail('Project').getAttribute('aria-pressed') === 'false');
    rail('Project').click();
    await until('the left area again', RUN_MS, () => document.querySelector('.area.left') !== null && rail('Project').getAttribute('aria-pressed') === 'true');
    steps.push('rail folds the left area');

    from = mark();
    v = await step('run again', () => api.run());
    await result(v, from, '2.5', BUILD_MS);
    await until('two ready nodes on the canvas', RUN_MS, () => document.querySelectorAll('.patina-node.ready').length === 2);
    steps.push('ready on the canvas');
    const timeOf = (id: string) => document.querySelector(`.react-flow__node[data-id="${id}"] .patina-node-time`)?.textContent ?? '';
    await until('the run time on each node', RUN_MS, () => [A, B].every((id) => /^Run \d+(\.\d+)? (us|ms|s)$/.test(timeOf(id))));
    steps.push(`run time on the nodes: ${timeOf(A)}, ${timeOf(B)}`);
    // The Debug tab shows a bar for each node call on the timeline of its thread (15.7).
    const bottomTab = (name: string) => [...document.querySelectorAll<HTMLElement>('[role=tab]')].find((t) => t.textContent === name)!;
    bottomTab('Debug').click();
    await until('a bar for each node in the Debug tab', RUN_MS, () => document.querySelectorAll('.debug-call:not(:disabled)').length >= 2);
    steps.push(`Debug tab: ${document.querySelector('.debug-lane header')?.textContent}`);
    bottomTab('Output').click();
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
      document.querySelector('.react-flow__pane')!.dispatchEvent(mouseWheel(deltaY, at.clientX, at.clientY));
    const near = nodeElements();
    const heights = () => [...document.querySelectorAll<HTMLElement>('.react-flow__node')].map((n) => n.offsetHeight).join(' ');
    const boxes = heights();
    let outs = 0;
    while (scale() >= 0.5) {
      const z = scale();
      wheel(240);
      outs++;
      await until('a smaller zoom', RUN_MS, () => scale() < z);
    }
    await until('the simple node form', RUN_MS, () =>
      document.querySelectorAll('.patina-row').length === 0 && !document.querySelector('.wire-value') && !document.querySelector('.wire-type'));
    if (document.querySelectorAll('.patina-node .title').length !== 2) throw new Error('a node at a far zoom has no title');
    if (heights() !== boxes) throw new Error(`the node heights are ${heights()} at a far zoom, not ${boxes}`);
    steps.push(`simple nodes at a far zoom: ${near} node elements near, ${nodeElements()} far`);
    for (; outs > 0; outs--) {
      const z = scale();
      wheel(-240);
      await until('a larger zoom', RUN_MS, () => scale() > z);
    }
    await until('the badge after the far zoom', RUN_MS, () => badge()?.textContent === '2.5' && document.querySelectorAll('.patina-row').length > 0);

    v = await checkWires(step, until, steps, v);
    v = await checkCanvas(step, until, steps, v);
    // While the graph is paused, an edit makes the value of the last run stale (15.3, 15.9).
    await step('pause', () => api.pause());
    v = await step('edit while paused', () => api.edit({ kind: 'SetParam', id: A, key: 'value', text: '3' }));
    edited(v);
    await until('the stale value on the wire', RUN_MS, () => badge()?.classList.contains('stale') === true && badge()?.textContent === '2.5').catch((e: Error) => {
      throw new Error(`${e.message} (badges ${[...document.querySelectorAll('.wire-value')].map((b) => `${b.textContent}:${b.className}`).join()})`);
    });
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
    await until('two matches for SHO', RUN_MS, () => options().join() === 'show,showchecklib');
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
    const dragFromPin = async (what: string) => {
      const pinEl = document.querySelector(`.react-flow__node[data-id="${B}"] .react-flow__handle[data-handleid="value"]`)!;
      const pinBox = pinEl.getBoundingClientRect();
      pinEl.dispatchEvent(new MouseEvent('mousedown', { bubbles: true, clientX: pinBox.left + pinBox.width / 2, clientY: pinBox.top + pinBox.height / 2 }));
      document.dispatchEvent(new MouseEvent('mousemove', { bubbles: true, ...at }));
      document.querySelector('.react-flow__pane')!.dispatchEvent(new MouseEvent('mouseup', { bubbles: true, ...at }));
      await until(`the palette after ${what}`, RUN_MS, () => !!search());
    };
    const fedFromPin = async (what: string) => {
      await until(`the ${what} node from the pin`, RUN_MS, () => !search() && newNode()?.querySelector('.title')?.textContent === 'number');
      const fed = newNode()!.getAttribute('data-id');
      v = await step(`undo the ${what} node and its wire`, () => api.undo());
      if (v.graph.node?.length !== 2 || v.graph.wire?.[0]?.from !== `${A}.out`) throw new Error(`one undo did not remove the node and its wire: ${JSON.stringify(v.graph.wire)}`);
      v = await step(`redo the ${what} node and its wire`, () => api.redo());
      if (v.graph.wire?.length !== 1 || v.graph.wire[0].from !== `${fed}.out` || v.graph.wire[0].to !== `${B}.value`) {
        throw new Error(`the new node does not feed the pin: ${JSON.stringify(v.graph.wire)}`);
      }
      v = await step(`undo the ${what} node from a pin`, () => api.undo());
    };
    await dragFromPin('a drag from a pin');
    // The show node has no output. The output of the built-in delay is generic, so it fits. The
    // subgraph of the Test mode step and the number node of the library have an f64 output.
    await until('only the nodes with an f64 output', RUN_MS, () => options().join() === 'number,numberchecklib,switch_checksubgraph,delaybuilt-in').catch((e: Error) => {
      throw new Error(`${e.message} (options ${options().join()})`);
    });
    search()!.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    await fedFromPin('added');
    // A library node from the palette of a drag gets the wire in the same undo step (6.2).
    await dragFromPin('a second drag from a pin');
    const libOption = () => [...document.querySelectorAll<HTMLElement>('.palette [role=option]')].find((o) => o.textContent === 'numberchecklib');
    await until('the library node in the palette of the drag', RUN_MS, () => !!libOption());
    libOption()!.click();
    await fedFromPin('library');
    steps.push('add a node and a library node from a pin');

    // Order wires (9.2): a drag from the order socket of A to the order socket of B connects them,
    // and Delete on the selected order wire removes it. Both are edits with undo.
    const orderPin = (id: string, side: string) => document.querySelector(`.react-flow__node[data-id="${id}"] .react-flow__handle.${side}[data-handleid="@order"]`)!;
    const mid = (el: Element, b = el.getBoundingClientRect()) => ({ clientX: b.left + b.width / 2, clientY: b.top + b.height / 2 });
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    orderPin(A, 'source').dispatchEvent(new MouseEvent('mousedown', { bubbles: true, ...mid(orderPin(A, 'source')) }));
    document.dispatchEvent(new MouseEvent('mousemove', { bubbles: true, ...mid(orderPin(B, 'target')) }));
    orderPin(B, 'target').dispatchEvent(new MouseEvent('mouseup', { bubbles: true, ...mid(orderPin(B, 'target')) }));
    await until('the order wire after a drag between the order sockets', RUN_MS, () => document.querySelector('.react-flow__edge-order') !== null);
    const orderHit = document.querySelector('.react-flow__edge-order .wire-hit')!;
    orderHit.dispatchEvent(new MouseEvent('click', { bubbles: true, ...mid(orderHit) }));
    await until('the selected order wire', RUN_MS, () => document.querySelector('.react-flow__edge-order.selected') !== null);
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Delete', bubbles: true }));
    await until('no order wire after Delete', RUN_MS, () => document.querySelector('.react-flow__edge-order') === null);
    v = await step('undo the removal of the order wire', () => api.undo());
    if (v.graph.order?.length !== 1 || v.graph.order[0].before !== A || v.graph.order[0].after !== B || v.graph.wire?.length !== 1) {
      throw new Error(`Delete changed more than the order wire: ${JSON.stringify([v.graph.order, v.graph.wire])}`);
    }
    v = await step('undo the order wire', () => api.undo());
    if (v.graph.order?.length) throw new Error('undo did not remove the order wire');
    steps.push('order wire connected and removed');

    // New node (15.2): the + of the Nodes group asks for a name. A used name shows its problem. A
    // new name makes the folder, and the Code tab shows its code.
    const nameField = () => document.querySelector<HTMLInputElement>('[aria-label="Node name"]');
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
    // New widget, New connection and New type (15.4): the same + makes a widget, a connection kind and a type crate.
    for (const [one, name] of [['widget', 'made-widget'], ['connection', 'made-bus'], ['type', 'made-type']]) {
      const field = () => document.querySelector<HTMLInputElement>(`[aria-label="${one[0].toUpperCase()}${one.slice(1)} name"]`);
      document.querySelector<HTMLButtonElement>(`[aria-label="New ${one}"]`)!.click();
      await until(`the ${one} name field`, RUN_MS, () => !!field());
      enterText(field()!, name);
      const shown = () => [...document.querySelectorAll('[aria-label=Project] .item')].some((b) => b.textContent === name);
      await until(`the new ${one} folder and its code`, RUN_MS, () => !field() && shown() && codeTab());
    }
    steps.push('make a widget, a connection and a type folder');

    // Add crate (15.4): a bad crate name shows its problem before cargo runs, so no network is used.
    const crateField = () => document.querySelector<HTMLInputElement>('[aria-label="Crate, widget or type name"]');
    [...document.querySelectorAll<HTMLButtonElement>('.code-bar .btn')].find((b) => b.textContent === 'Add crate')!.click();
    await until('the crate field', RUN_MS, () => !!crateField());
    enterText(crateField()!, '-x');
    const crateProblem = () => document.querySelector('.new-item [role=alert]')?.textContent ?? '';
    await until('the problem of a bad crate name', RUN_MS, () => crateProblem().includes('name@version'));
    crateField()!.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    await until('the closed crate field', RUN_MS, () => !crateField());
    steps.push('refuse a bad crate name');
    // Undo removes a new folder that has only its start files, and redo makes it again (15.2).
    const typeListed = () => [...document.querySelectorAll('[aria-label=Project] .item')].some((b) => b.textContent === 'made-type');
    const typeShown = () => document.querySelector('[aria-label=Inspector] .panel-title')?.textContent === 'made-type';
    v = await step('undo the new type folder', () => api.undo());
    await until('the type folder that undo removes', RUN_MS, () => !typeListed() && !typeShown());
    v = await step('redo the new type folder', () => api.redo());
    await until('the type folder that redo makes again', RUN_MS, () => typeListed() && typeShown() && codeTab());
    steps.push('undo and redo a new folder');
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

    v = await checkDomains({ step, until, steps, mark, events, fresh: () => output.slice(outputFrom), topButton, statusText }, v, lib, changedCode);
    const ofRun = () => events.slice(from).filter((e) => e.serial === v.serial);
    const eventRuns = () => ofRun().flatMap((e) => (e.state === 'fired' ? [e] : []));

    // A node GUI (4.3): the node body shows the button of the `ui`. A click in a live graph runs
    // the graph again with the new state, and does not build again.
    v = await step('make a node with a GUI', () => api.edit({ kind: 'SetCode', path: lib, text: GUI_NODE }));
    from = mark();
    v = await step('run the node GUI', () => api.run());
    await result(v, from, '2.5', BUILD_MS);
    const guiButton = (text: string) =>
      [...document.querySelectorAll<HTMLButtonElement>(`.react-flow__node[data-id="${A}"] .node-gui button`)].find((b) => b.textContent === text);
    await until('the button of the node GUI', RUN_MS, () => guiButton('Add 1')?.disabled === false);
    from = mark();
    guiButton('Add 1')!.click();
    const clicked = await result(v, from, '3.5', RUN_MS);
    if (clicked.builtMs !== null) throw new Error('a click in a node GUI built the graph again');
    // A node GUI writes its own parameter through the edit path, so undo removes the write.
    from = mark();
    guiButton('Set 4')!.click();
    await result(v, from, '5', RUN_MS);
    from = mark();
    v = await step('undo the parameter write of the node GUI', () => api.undo());
    await result(v, from, '3.5', RUN_MS);
    if (v.graph.node?.find((n) => n.id === A)?.params?.value !== '2.5') throw new Error('undo did not remove the parameter write of the node GUI');
    const gui = `.react-flow__node[data-id="${A}"] .node-gui`;
    await checkWidgets(until, gui);
    await checkGuard(until);
    await api.capture('node-gui');
    tab('Parameters')!.click();
    await until('the large node GUI in the Inspector', RUN_MS, () => document.querySelector('aside[aria-label="Inspector"] .node-gui .gui-image') !== null);
    await api.capture('node-gui-large');
    // The expand button opens the full size, the node draws it in the full mode, and Esc closes it.
    document.querySelector<HTMLButtonElement>(`.react-flow__node[data-id="${A}"] button[aria-label="Open the full size"]`)!.click();
    const full = () => document.querySelector<HTMLElement>('.full-gui');
    await until('the full size of the node GUI', RUN_MS, () => full()?.textContent?.includes('Full size') === true);
    if (document.querySelector(gui)?.textContent?.includes('Full size')) throw new Error('the node body shows the full size');
    await api.capture('node-gui-full');
    full()!.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    await until('the closed full size', RUN_MS, () => !full());
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
    const viewButton = (text: string) => [...document.querySelectorAll<HTMLButtonElement>(`${panel} .node-gui button`)].find((b) => b.textContent === text);
    viewButton('Set')!.click();
    await until('the write of the view', RUN_MS, () => viewText().startsWith('value 5Set'));
    await api.capture('view');
    v = await step('undo the view write', () => api.undo());
    if (v.graph.node?.find((n) => n.id === A)?.params?.value === '5') throw new Error('undo did not remove the write of the view');
    steps.push('a view sets a parameter');

    // A view call that does not return names no node. The second Stop ends it too (16.1).
    v = await step('a value that the view does not return at', () => api.edit({ kind: 'SetParam', id: A, key: 'value', text: '33' }));
    v = await step('run with the view that does not return', () => api.run());
    // Stop before the hang ends the watch of the status bar, so wait for the view call.
    for (const end = Date.now() + RUN_MS; (await api.running()) !== ''; ) {
      if (Date.now() > end) throw new Error(`no result after ${RUN_MS} ms: the view call that does not return`);
      await new Promise((r) => setTimeout(r, 100));
    }
    await until('the Stop button for the view', RUN_MS, () => topButton('Stop')?.disabled === false);
    topButton('Stop')!.click();
    await until('the view that does not stop in the status bar', RUN_MS, () => statusText().includes('or a view does not stop'));
    topButton('Stop')!.click();
    const viewEnded = 'Patina ended the node process, because a node `Default`, a node `Drop` or a view did not stop.';
    await until('the end of the node process for the view', RUN_MS, () => statusText().includes(viewEnded));
    v = await step('undo the value that the view does not return at', () => api.undo());
    v = await step('run in a new node process for the view', () => api.run());
    const again = () => document.querySelector<HTMLButtonElement>(`${panel} .btn`);
    await until('the Try again button of the view', RUN_MS, () => !!again());
    again()!.click();
    // The old error shows until the page draws again after the click.
    await until('the widgets of the view in a new node process', BUILD_MS, () => !document.querySelector(`${panel} .error-text`) && viewText().startsWith('value '));
    steps.push('the second Stop ends a view that does not return');

    // A view fires a `trigger` node of the live run with a text (9.4, 15.8).
    v = await step('make an event node for the view', () => api.edit({ kind: 'SetCode', path: lib, text: EVENT_NODE }));
    v = await step('add a trigger for the view', () =>
      api.edit({
        kind: 'Group',
        edits: [
          { kind: 'AddNode', id: T, folder: 'trigger', builtin: true, pos: [-300, 0] },
          { kind: 'Connect', from: `${T}.out`, to: `${A}.text` },
        ],
      }),
    );
    from = mark();
    v = await step('run the triggers for the view', () => api.run());
    await until('a run with triggers that listen for the view', BUILD_MS, () => {
      const failed = ofRun().find((e) => e.state === 'failed' || (e.state === 'ran' && !e.ok));
      if (failed) throw new Error(`the run failed: ${JSON.stringify(failed)}`);
      return ofRun().some((e) => e.state === 'ran' && e.events);
    });
    viewButton('Fire')!.click();
    await until('the event run of the view that prints 6.5', RUN_MS, () => viewText() !== '' && output.slice(outputFrom).includes('6.5'));
    topButton('Stop')!.click();
    await until('the end of the event runs of the view', RUN_MS, () => ofRun().some((e) => e.state === 'ended'));
    v = await step('undo the trigger of the view', () => api.undo());
    v = await step('undo the event node of the view', () => api.undo());
    steps.push('a view fires a trigger node');
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

    // Fire in a subgraph use (15.8): Open subgraph opens the subgraph through its use, and Fire
    // sends the flat ID <use ID>/<node ID>. This step is last, because the change of graph saves.
    const use = 'f12e000000000001';
    v = await step('make an event node for the subgraph', () => api.edit({ kind: 'SetCode', path: lib, text: EVENT_NODE }));
    v = await step('add a trigger for the subgraph', () =>
      api.edit({
        kind: 'Group',
        edits: [
          { kind: 'AddNode', id: T, folder: 'trigger', builtin: true, pos: [-300, 0] },
          { kind: 'Connect', from: `${T}.out`, to: `${A}.text` },
        ],
      }),
    );
    v = await step('move the trigger into a subgraph', () => api.collapse('fire_check', [T], use));
    from = mark();
    v = await step('run the trigger in the subgraph', () => api.run());
    await until('a run with the trigger in the subgraph', BUILD_MS, () => {
      const failed = ofRun().find((e) => e.state === 'failed' || (e.state === 'ran' && !e.ok));
      if (failed) throw new Error(`the run failed: ${JSON.stringify(failed)}`);
      return ofRun().some((e) => e.state === 'ran' && e.events);
    });
    await until('the use of the subgraph', RUN_MS, () => !!document.querySelector(`.react-flow__node[data-id="${use}"]`));
    document.querySelector<HTMLButtonElement>(`.react-flow__node[data-id="${use}"] .open-subgraph`)!.click();
    await until('the use in the path bar', BUILD_MS, () => document.querySelector('.graph-path [aria-current="page"]')?.textContent === '/ fire_check');
    await until('the fire button in the subgraph', BUILD_MS, () => fireButton()?.disabled === false);
    fireButton()!.click();
    await until('the event run of the trigger in the use', RUN_MS, () => eventRuns().some((e) => e.trigger === `${use}/${T}`) && output.slice(outputFrom).includes('2.5'));
    topButton('Stop')!.click();
    await until('the end of the event runs in the subgraph', RUN_MS, () => ofRun().some((e) => e.state === 'ended'));
    // The Subgraph interface (mockup 11): the Inspector lists the public pins in pin order (6.4).
    const rows = () => [...document.querySelectorAll<HTMLElement>('aside[aria-label="Inspector"] .pin-row')];
    // A view tab can cover the Inspector. The click on Fire selected the trigger, and a click on the
    // pane clears the selection. With selection on drag, the pane takes a pointer down and up.
    changeLayout((l) => openPanel(l, 'inspector'));
    const flow = document.querySelector('.react-flow__pane')!;
    const corner = flow.getBoundingClientRect();
    for (const type of ['pointerdown', 'pointerup']) flow.dispatchEvent(new PointerEvent(type, { bubbles: true, button: 0, pointerId: 1, isPrimary: true, clientX: corner.left + 5, clientY: corner.top + 5 }));
    await until('the subgraph interface', RUN_MS, () => rows().length === 1);
    const firstPin = rows()[0].dataset.pin;
    [...document.querySelectorAll<HTMLButtonElement>('aside[aria-label="Inspector"] .btn')].find((b) => b.textContent === 'Add output')!.click();
    await until('a new public output', BUILD_MS, () => rows().length === 2);
    const added = rows()[1].dataset.pin;
    rows()[1].querySelector<HTMLButtonElement>('button[title="Move up"]')!.click();
    await until('the new output first in the pin order', BUILD_MS, () => rows()[0].dataset.pin === added);
    const field = rows()[0].querySelector('input')!;
    field.value = 'sum';
    field.dispatchEvent(new FocusEvent('focusout', { bubbles: true }));
    await until('the new name in the preview', BUILD_MS, () => document.querySelector('.pin-preview-body > :last-child > :first-child')?.textContent === 'sum');
    for (const what of ['name', 'order', 'output']) v = await step(`undo the ${what} of the public output`, () => api.undo());
    await until('the old public pins', BUILD_MS, () => rows().length === 1 && rows()[0].dataset.pin === firstPin);
    steps.push('subgraph interface');
    [...document.querySelectorAll<HTMLButtonElement>('.graph-path button')].find((b) => b.textContent === 'main')!.click();
    await until('the main graph from the path bar', BUILD_MS, () => document.querySelector('.graph-path') === null);
    // The only use of a subgraph gives its values when it opens from the Project panel (15.9).
    leftTab('Project').click();
    [...document.querySelectorAll<HTMLButtonElement>('[aria-label=Project] .item')].find((b) => b.textContent === 'fire_check')!.click();
    await until('the domain of the only use', BUILD_MS, () => document.querySelector('.graph-path') !== null && !!document.querySelector(`.react-flow__node[data-id="${T}"] .domain-icon`));
    [...document.querySelectorAll<HTMLButtonElement>('.graph-path button')].find((b) => b.textContent === 'main')!.click();
    await until('the main graph again', BUILD_MS, () => document.querySelector('.graph-path') === null);
    steps.push('open a subgraph use and fire its trigger');

    // Show inner nodes puts the nodes of the use in place, in a frame, with the wire to the frame pin (15.3).
    const frameButton = () => document.querySelector<HTMLButtonElement>(`.react-flow__node[data-id="${use}"] .frame-toggle`);
    const innerNode = () => document.querySelector(`.react-flow__node[data-id="${use}/${T}"]`);
    await until('the frame button of the use', BUILD_MS, () => frameButton()?.getAttribute('aria-label') === 'Show inner nodes');
    // The open frame moves the other nodes aside, so it covers none of them. They come back when it closes.
    const others = () => [...document.querySelectorAll<HTMLElement>('.react-flow__node')].filter((n) => n.dataset.id !== use && !n.dataset.id?.startsWith(`${use}/`));
    const places = () => others().map((n) => `${n.dataset.id} ${n.style.transform}`).join('\n');
    const covered = () => {
      const f = document.querySelector(`.react-flow__node[data-id="${use}"]`)!.getBoundingClientRect();
      return others().some((n) => {
        const r = n.getBoundingClientRect();
        return r.left < f.right - 1 && r.right > f.left + 1 && r.top < f.bottom - 1 && r.bottom > f.top + 1;
      });
    };
    const before = places();
    frameButton()!.click();
    await until('the inner node and its wire to the frame pin', BUILD_MS, () => !!innerNode() && !!document.querySelector(`.react-flow__edge[data-id^="${use}/${T}.out>${use}.out:"]`));
    await until('an open frame that covers no node', BUILD_MS, () => !covered());
    if (innerNode()!.classList.contains('draggable') || innerNode()!.querySelector('.frame-toggle, .open-subgraph')) throw new Error('an inner node has edits');
    frameButton()!.click();
    await until('no inner nodes', RUN_MS, () => !innerNode() && frameButton()?.getAttribute('aria-expanded') === 'false');
    await until('the nodes in their places again', BUILD_MS, () => places() === before);
    steps.push('show the inner nodes of a use in a frame');

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

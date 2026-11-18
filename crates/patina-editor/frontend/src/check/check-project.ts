// The project steps of the self-check (check.ts): a run, a live edit, undo, compile errors, the
// Code tab, the slot, a panic, Stop and the recovery file (7.1, 15.2, 15.3, 15.10).

import { api, type EngineEvent, type View } from '../api';
import { bufferOf, flushCode, monaco, openFile, unsavedFiles } from '../code/code';
import { A, B, BUILD_MS, RUN_MS, button, enterText, helloNodes, leftTab, statusText, tab, topButton, type CheckContext } from './check-fixtures';

/** Makes the projects and runs the first steps. It returns the last view, and the code file of node A with its first code. */
export async function checkProject({ step, until, result, mark, events, steps, fresh }: CheckContext, dir: string) {
  // A second new project is a library of the check: the palette shows its nodes (6.2).
  const libProject = await step('new library project', () => api.newProject(dir, 'checklib'));
  await helloNodes();
  // Add library in Settings adds the library project. The check gives the folder in place of the folder dialog.
  topButton('Close')!.click();
  await until('the Settings button of the start screen', RUN_MS, () => !!button('Settings'));
  button('Settings')!.click();
  const addLib = () => [...document.querySelectorAll<HTMLButtonElement>('[aria-label="Libraries"] button')].find((b) => b.textContent === 'Add library');
  await until('the Add library button', RUN_MS, () => addLib()?.disabled === false);
  const pickFolder = api.pickFolder;
  api.pickFolder = async () => libProject.root;
  try {
    addLib()!.click();
    const norm = (p: string) => p.replace(/\\/g, '/').toLowerCase();
    await until('the row of the new library', RUN_MS, () =>
      [...document.querySelectorAll('[aria-label="Libraries"] .path')].some((p) => norm(p.textContent ?? '') === norm(libProject.root)),
    );
  } finally {
    api.pickFolder = pickFolder;
  }
  button('Back')!.click();
  // The Create button of the start screen makes the project of the check (15.10). Pause gives its view.
  const newForm = () => [...document.querySelectorAll('form.start-box')].find((f) => f.querySelector('h2')?.textContent === 'New project');
  const create = () => [...(newForm()?.querySelectorAll('button') ?? [])].find((b) => b.textContent === 'Create');
  await until('the New project form', RUN_MS, () => !!create());
  const [folder, name] = newForm()!.querySelectorAll('input');
  enterText(folder, dir);
  enterText(name, 'check');
  await until('the Create button on', RUN_MS, () => create()?.disabled === false);
  create()!.click();
  await until('the new project in the editor', BUILD_MS, () => !newForm() && !!topButton('Run'));
  let v = await step('new project', () => api.pause());
  if (v.root.replace(/\\/g, '/').split('/').pop() !== 'check') throw new Error(`the Create button made ${v.root}, not the check project`);
  if (!v.trusted || v.live || (v.graph.node ?? []).length !== 0 || v.nodes.some((n) => !n.builtin && !n.subgraph)) throw new Error('a new project is trusted, paused and has no nodes');
  await helloNodes();
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
  // The compile error link of the status bar opens the first error in the Code tab.
  const errorLink = () => [...document.querySelectorAll<HTMLButtonElement>('.status-link')].find((b) => b.textContent === '1 compile error');
  await until('the error on the node and in the status bar', RUN_MS, () => document.querySelector('.patina-node.compile-error') !== null && !!errorLink());
  errorLink()!.click();
  await until('the file in the Code tab', RUN_MS, () => bufferOf(lib) !== undefined && document.querySelector('.monaco-editor') !== null);
  await until('the error mark in the gutter', RUN_MS, () => document.querySelector('.gutter-error') !== null);
  steps.push('compile error shown');
  // The Logs panel highlights the error and adds a Fix line from the list of common errors (15.8).
  const shown = tab('Logs')!.parentElement?.querySelector<HTMLElement>('[role=tab][aria-selected=true]');
  tab('Logs')!.click();
  const logs = () => document.querySelector('[role=log][aria-label=Logs]');
  await until('the highlight and the Fix line of the error in the Logs panel', RUN_MS, () =>
    !!logs()?.querySelector('.hl-sev-err') && [...(logs()?.querySelectorAll('.hl-fix') ?? [])].some((f) => f.parentElement?.textContent?.includes('another type')),
  );
  // Clear in the header of the Logs panel removes the lines.
  [...tab('Logs')!.parentElement!.querySelectorAll<HTMLButtonElement>('button')].find((b) => b.textContent === 'Clear')!.click();
  await until('the empty Logs panel after Clear', RUN_MS, () => logs()?.textContent === '');
  shown?.click();
  steps.push('the Logs panel highlights an error and gives a fix, and Clear removes the lines');
  await new Promise((r) => setTimeout(r, 300));
  await api.capture('code-error');

  // The Code tab keeps the cursor of each file when the file or the Inspector tab changes (15.10).
  const codeEditor = () => monaco.editor.getEditors()[0];
  const lineAt = () => codeEditor()?.getPosition()?.lineNumber;
  const tabOf = (box: string, name: (t: HTMLElement) => boolean) => [...document.querySelectorAll<HTMLElement>(`${box} [role=tab]`)].find(name)!;
  tabOf('.code-tab .files', (t) => t.title.endsWith('/Cargo.toml')).click();
  await until('Cargo.toml in the Code tab', RUN_MS, () => codeEditor()?.getModel()?.uri.path.endsWith('/Cargo.toml') === true);
  // The Code tab highlights TOML, so the lines of Cargo.toml have more than two token colors.
  const colors = () => new Set([...document.querySelectorAll('.code-box .view-line span[class^=mtk]')].map((s) => s.className)).size;
  await until('the TOML highlight of Cargo.toml', RUN_MS, () => codeEditor()?.getModel()?.getLanguageId() === 'toml' && colors() > 2);
  steps.push('the Code tab highlights TOML');
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
  // The Save button of the Code tab is on only while a file of the node has changes.
  const saveButton = () => [...document.querySelectorAll<HTMLButtonElement>('.code-tab .code-bar .btn')].find((b) => b.textContent === 'Save');
  await until('the slot error in the Code tab', RUN_MS, () => bufferOf(lib)?.getValue() === slotError && saveButton()?.disabled === true);
  bufferOf(lib)!.setValue(stoppable);
  await until('the Save button on', RUN_MS, () => saveButton()?.disabled === false);
  from = mark();
  saveButton()!.click();
  const slotFixed = await result(v, from, '2.5', BUILD_MS);
  if (!slotFixed.slotOnly || slotFixed.errors.length > 0) throw new Error(`the save after a slot error built all: ${JSON.stringify(slotFixed.errors)}`);
  await until('no compile error on the node and the Save button off', RUN_MS, () =>
    document.querySelector('.patina-node.compile-error') === null && saveButton()?.disabled === true,
  );
  steps.push('fix the slot with the Save button');

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
  await until('the Stop button', RUN_MS, () => topButton('Stop')?.disabled === false);
  topButton('Stop')!.click();
  const ended = await endOf(v, from);
  if (ended.state !== 'ran' || !ended.ok || ended.stopped.join() !== B || fresh().includes('99')) {
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
  return { v, lib, original };
}

/** Restores a graph change and a code change after a stop of Patina, and returns the changed code. */
export async function checkRecovery({ step, until, steps }: CheckContext, v: View, lib: string, original: string) {
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
  // Discard removes the recovery file and keeps the saved project.
  v = await step('change again without a save', () => api.edit({ kind: 'MoveNode', id: B, pos: [bx, by + 80] }));
  v = await step('open after a second stop', () => api.openProject(v.root));
  if (!v.recovery) throw new Error('the second open did not offer the recovery file');
  const discard = () => [...document.querySelectorAll<HTMLButtonElement>('.dialog button')].find((b) => b.textContent === 'Discard');
  await until('the Discard button', RUN_MS, () => discard() !== undefined);
  discard()!.click();
  await until('no recovery dialog after Discard', RUN_MS, () => !discard() && !dirtyDot());
  v = await step('open after the discard', () => api.openProject(v.root));
  if (v.recovery || JSON.stringify(v.graph) !== changedGraph) throw new Error('the discard did not keep the saved graph or remove the recovery file');
  steps.push('recovery file discarded');
  return { v, changedCode };
}

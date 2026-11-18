// The subgraph steps of the self-check (check.ts): a trigger in a subgraph use, the Subgraph
// interface, the frame of the inner nodes, a node GUI in a use and Collapse from the Inspector
// (4.3, 6.1, 6.4, 15.3, 15.4, 15.8, 15.9).

import { api, type View } from '../api';
import { changeLayout } from '../panels/Dock';
import { openPanel } from '../panels/layout';
import { clipboard } from '../canvas/clip';
import { A, BUILD_MS, EVENT_NODE, RUN_MS, T, button, closeMenu, enterText, entry, fireButton, leftTab, nodeToml, rightClick, topButton, type CheckContext } from './check-fixtures';

/** A node with a GUI: a switch and a text that shows the state of the switch. */
const LAMP_NODE = `use patina_sdk::node;

#[node]
pub struct Lamp {
    on: bool,
}

impl Lamp {
    pub fn init() -> Self {
        Lamp { on: false }
    }

    pub fn process(&mut self, value: f64) -> f64 {
        if self.on { value } else { 0.0 }
    }

    pub fn ui(&mut self, ui: &mut patina_sdk::Ui) {
        ui.toggle("Power", &mut self.on);
        ui.label(if self.on { "Lamp on" } else { "Lamp off" });
    }
}
`;

/** Runs the subgraph steps with the code file `lib` of node A. They are last, because the change of graph saves. */
export async function checkSubgraph({ step, until, mark, events, steps, fresh }: CheckContext, v: View, lib: string) {
  let from = 0;
  const ofRun = () => events.slice(from).filter((e) => e.serial === v.serial);
  const eventRuns = () => ofRun().flatMap((e) => (e.state === 'fired' ? [e] : []));
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
  await until('the event run of the trigger in the use', RUN_MS, () => eventRuns().some((e) => e.trigger === `${use}/${T}`) && fresh().includes('2.5'));
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
  rows()[0].querySelector<HTMLButtonElement>('button[title="Move down"]')!.click();
  await until('the new output last in the pin order after Move down', BUILD_MS, () => rows()[1].dataset.pin === added);
  for (const what of ['move down', 'name', 'order', 'output']) v = await step(`undo the ${what} of the public output`, () => api.undo());
  await until('the old public pins', BUILD_MS, () => rows().length === 1 && rows()[0].dataset.pin === firstPin);
  steps.push('subgraph interface');
  // Rename pin... in the menu of a public pin asks for the name in a field (6.4).
  const portNode = () => document.querySelector(`.react-flow__node[data-id="out:${firstPin}"] .title`);
  const oldName = portNode()!.textContent!;
  rightClick(portNode()!);
  await until('the Rename pin entry', RUN_MS, () => !!entry('Node', 'Rename pin...'));
  entry('Node', 'Rename pin...')!.click();
  const pinField = () => document.querySelector<HTMLInputElement>('[role="dialog"][aria-label="Rename the public pin"] input[aria-label="Pin name"]');
  await until('the pin name field with the old name', RUN_MS, () => pinField()?.value === oldName);
  enterText(pinField()!, 'total');
  await until('the new pin name on the canvas', BUILD_MS, () => !pinField() && portNode()?.textContent === 'total');
  v = await step('undo the rename of the public pin', () => api.undo());
  if (v.graph.output?.[0]?.name !== oldName) throw new Error(`the undo gave the pin name ${v.graph.output?.[0]?.name}, not ${oldName}`);
  await until('the old pin name on the canvas', BUILD_MS, () => portNode()?.textContent === oldName);
  steps.push('rename a public pin from its menu');
  // A removed public pin keeps the wire of the use. The use shows a lost pin for it, so the user can delete the wire (15.9).
  rows()[0].querySelector<HTMLButtonElement>('button[title="Remove"]')!.click();
  await until('no public output', BUILD_MS, () => rows().length === 0);
  [...document.querySelectorAll<HTMLButtonElement>('.graph-path button')].find((b) => b.textContent === 'main')!.click();
  await until('the main graph from the path bar', BUILD_MS, () => document.querySelector('.graph-path') === null);
  const lostPin = `.react-flow__node[data-id="${use}"] .react-flow__handle.lost-pin[data-handleid="${firstPin}"]`;
  await until('the lost pin of the use and its wire', BUILD_MS, () => !!document.querySelector(lostPin) && !!document.querySelector(`.react-flow__edge[data-id="${use}.${firstPin}>${A}.text"]`));
  // The only use of a subgraph gives its values when it opens from the Project panel (15.9).
  leftTab('Project').click();
  [...document.querySelectorAll<HTMLButtonElement>('[aria-label=Project] .item')].find((b) => b.textContent === 'fire_check')!.click();
  await until('the subgraph from the Project panel', BUILD_MS, () => document.querySelector('.graph-path') !== null);
  v = await step('undo the removed public output', () => api.undo());
  if (v.graph.output?.[0]?.key !== firstPin) throw new Error(`the undo did not give back the public output ${firstPin}`);
  await until('the domain of the only use', BUILD_MS, () => !!document.querySelector(`.react-flow__node[data-id="${T}"] .domain-icon`));
  [...document.querySelectorAll<HTMLButtonElement>('.graph-path button')].find((b) => b.textContent === 'main')!.click();
  await until('the main graph again', BUILD_MS, () => document.querySelector('.graph-path') === null);
  steps.push('open a subgraph use and fire its trigger');

  // A node folder that comes or goes from outside the editor shows in the Project panel (15.10).
  const outside = () => [...document.querySelectorAll<HTMLButtonElement>('[aria-label=Project] .item')].find((b) => b.textContent?.startsWith('outside'));
  await api.checkWrite('nodes/outside/src/lib.rs', '#[patina_sdk::node]\npub fn outside(a: f64) -> f64 {\n    a\n}\n');
  await api.checkWrite('nodes/outside/Cargo.toml', nodeToml('outside'));
  leftTab('Project').click();
  await until('the node from outside the editor', BUILD_MS, () => !!outside());
  await api.checkWrite('nodes/outside', null);
  await until('no node from outside the editor', BUILD_MS, () => !outside());
  steps.push('a node folder from outside the editor');

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

  // A built-in node has no folder, so the Nodes group of the Project panel does not list it (15.4).
  const items = () => [...document.querySelectorAll<HTMLButtonElement>('[aria-label=Project] .item')];
  const item = (name: string) => items().find((b) => b.textContent?.startsWith(name));
  await api.checkWrite('nodes/lamp/Cargo.toml', nodeToml('lamp'));
  await api.checkWrite('nodes/lamp/src/lib.rs', LAMP_NODE);
  leftTab('Project').click();
  await until('the lamp node folder', BUILD_MS, () => !!item('lamp'));
  const builtins = v.nodes.filter((n) => n.builtin).map((n) => n.name);
  const listed = builtins.filter((name) => items().some((b) => b.textContent === name));
  if (builtins.length === 0 || listed.length > 0) throw new Error(`the Project panel lists the built-in nodes ${listed.join()} of ${builtins.join()}`);
  steps.push('no built-in nodes in the Project panel');
  // The right-click menu of a node folder inspects its code (15.4).
  const inspector = 'aside[aria-label="Inspector"]';
  const inspectorTitle = () => document.querySelector(`${inspector} .panel-title`)?.textContent ?? '';
  const inspectorTabs = () => [...document.querySelectorAll<HTMLButtonElement>(`${inspector} [role=tab]`)];
  changeLayout((l) => openPanel(l, 'inspector'));
  rightClick(item('lamp')!);
  await until('the menu of the node folder', RUN_MS, () => !!entry('nodes/lamp', 'Inspect') && !!entry('nodes/lamp', 'Delete'));
  entry('nodes/lamp', 'Inspect')!.click();
  const tabNames = () => inspectorTabs().map((t) => t.textContent ?? '');
  // A node folder shows its code and its files, but no Parameters tab.
  await until('the code of the node folder', RUN_MS, () => inspectorTitle() === 'lamp' && tabNames().includes('src/lib.rs') && !tabNames().includes('Parameters')).catch((e: Error) => {
    throw new Error(`${e.message} (title ${inspectorTitle()}, tabs ${tabNames().join()})`);
  });
  steps.push('inspect a node folder from its menu');
  // Delete in the same menu asks first. Cancel keeps the folder, so the check puts nothing in the Recycle Bin.
  rightClick(item('lamp')!);
  await until('the Delete entry of the node folder', RUN_MS, () => !!entry('nodes/lamp', 'Delete'));
  entry('nodes/lamp', 'Delete')!.click();
  const asked = () => document.querySelector('.dialog[role="dialog"]')?.textContent ?? '';
  await until('the delete question of the node folder', RUN_MS, () => asked().includes('Delete nodes/lamp?') && asked().includes('Recycle Bin') && !!button('Cancel'));
  button('Cancel')!.click();
  await until('the node folder after Cancel', RUN_MS, () => !asked() && !!item('lamp'));
  steps.push('cancel the Delete of a node folder from its menu');
  // Import in the Project panel lists the folders of another project, and Cancel copies nothing (6.2).
  // The check gives the library folder in place of the folder dialog.
  const libRoot = (await api.settings()).libraries.find((l) => /[\\/]checklib$/.test(l.path))!.path;
  const pickFolder = api.pickFolder;
  api.pickFolder = async () => libRoot;
  const importBox = () => document.querySelector('.dialog.import');
  try {
    button('Import')!.click();
    await until('the Import dialog of the library', RUN_MS, () => importBox()?.querySelector('h2')?.textContent === 'Import from checklib');
  } finally {
    api.pickFolder = pickFolder;
  }
  const folderList = importBox()!.querySelector<HTMLSelectElement>('select[aria-label="Folder to import"]')!;
  await until('the node folders of the library', RUN_MS, () => folderList.options.length > 1);
  folderList.value = folderList.options[1].value;
  folderList.dispatchEvent(new Event('change', { bubbles: true }));
  await until('the plan of the import', RUN_MS, () => !!importBox()?.querySelector('.import-items li'));
  [...importBox()!.querySelectorAll<HTMLButtonElement>('button')].find((b) => b.textContent === 'Cancel')!.click();
  await until('the closed Import dialog', RUN_MS, () => !importBox());
  steps.push('the Import dialog lists the folders of another project, and Cancel closes it');
  // nodes/number of this project has other code, so the import of the library folder needs a rename (6.2).
  // The copy goes in the Imports list, and its Update dialog shows that the library did not change.
  const dialogButton = (box: Element | null, label: string) => [...(box?.querySelectorAll<HTMLButtonElement>('button') ?? [])].find((b) => b.textContent === label);
  const imported = () => [...document.querySelectorAll<HTMLButtonElement>('[aria-label=Project] .item')].find((b) => b.title.startsWith('Compare') && b.textContent?.startsWith('nodes/number-2'));
  const pick = (select: HTMLSelectElement, value: string) => {
    select.value = value;
    select.dispatchEvent(new Event('change', { bubbles: true }));
  };
  api.pickFolder = async () => libRoot;
  try {
    button('Import')!.click();
    const list = () => importBox()?.querySelector<HTMLSelectElement>('select[aria-label="Folder to import"]');
    await until('nodes/number in the Import dialog', RUN_MS, () => [...(list()?.options ?? [])].some((o) => o.value === 'nodes/number'));
    pick(list()!, 'nodes/number');
    const choice = () => importBox()?.querySelector<HTMLSelectElement>('select[aria-label="Choice for nodes/number"]');
    await until('the conflict of nodes/number', RUN_MS, () => !!choice());
    pick(choice()!, 'Rename');
    await until('the name number-2', RUN_MS, () => importBox()?.querySelector<HTMLInputElement>('input[aria-label="New name for nodes/number"]')?.value === 'number-2');
    dialogButton(importBox(), 'Import')!.click();
    await until('the copy in the Imports list', BUILD_MS, () => !importBox() && !!imported()).catch((e: Error) => {
      throw new Error(`${e.message} (${importBox()?.querySelector('.error-text')?.textContent ?? 'no error'})`);
    });
    imported()!.click();
    const unchanged = () => importBox()?.querySelector('h2')?.textContent === 'Update nodes/number-2 from checklib' && !!importBox()?.textContent?.includes('The folder in the other project did not change.');
    await until('the Update dialog of the copy', RUN_MS, unchanged);
    if (!dialogButton(importBox(), 'Update')?.disabled) throw new Error('Update is on with no change in the other project');
    dialogButton(importBox(), 'Cancel')!.click();
    await until('the closed Update dialog', RUN_MS, () => !importBox());
  } finally {
    api.pickFolder = pickFolder;
  }
  await api.checkWrite('nodes/number-2', null);
  await until('no copy after a delete from outside the editor', BUILD_MS, () => !imported());
  steps.push('import a library folder with a new name, open its Update dialog, and delete the copy');

  // A node GUI in a subgraph use shows and takes input, on the canvas and in the Inspector (4.3, 6.4).
  const lamp = 'f12e000000000002';
  document.querySelector<HTMLButtonElement>(`.react-flow__node[data-id="${use}"] .open-subgraph`)!.click();
  await until('the use in the path bar for the lamp', BUILD_MS, () => document.querySelector('.graph-path [aria-current="page"]')?.textContent === '/ fire_check');
  v = await step('add a lamp to the subgraph', () => api.edit({ kind: 'AddNode', id: lamp, folder: 'nodes/lamp', pos: [-300, 150] }));
  from = mark();
  v = await step('run the lamp in the subgraph use', () => api.run());
  await until('a run with the lamp in the use', BUILD_MS, () => {
    const failed = ofRun().find((e) => e.state === 'failed' || (e.state === 'ran' && !e.ok));
    if (failed) throw new Error(`the run failed: ${JSON.stringify(failed)}`);
    return ofRun().some((e) => e.state === 'ran');
  });
  rightClick(document.querySelector('.react-flow__pane')!);
  await until('the canvas menu in the subgraph', RUN_MS, () => !!entry('Canvas', 'Fit view'));
  entry('Canvas', 'Fit view')!.click();
  const onCanvas = `.react-flow__node[data-id="${lamp}"]`;
  const gui = (where: string) => document.querySelector(`${where} .node-gui`);
  const shows = (where: string, text: string) => gui(where)?.textContent?.includes(text) === true;
  const flip = (where: string) => gui(where)!.querySelector<HTMLInputElement>('input[type=checkbox]')!.click();
  await until('the lamp GUI in the use', BUILD_MS, () => shows(onCanvas, 'Lamp off'));
  flip(onCanvas);
  await until('the lamp on after a click on its switch', RUN_MS, () => shows(onCanvas, 'Lamp on'));
  rightClick(document.querySelector(`${onCanvas} .title`)!);
  await until('the node menu of the lamp', RUN_MS, () => !!entry('Node', 'Delete node'));
  closeMenu();
  await until('the lamp in the Inspector', RUN_MS, () => inspectorTitle().includes('lamp') && tabNames().includes('Parameters'));
  inspectorTabs().find((t) => t.textContent === 'Parameters')!.click();
  await until('the lamp GUI in the Inspector', BUILD_MS, () => shows(inspector, 'Lamp on'));
  flip(inspector);
  await until('the lamp off in the Inspector and on the canvas', RUN_MS, () => shows(inspector, 'Lamp off') && shows(onCanvas, 'Lamp off'));
  topButton('Stop')!.click();
  await until('the end of the lamp run', RUN_MS, () => ofRun().some((e) => e.state === 'ended'));
  steps.push('a node GUI in a subgraph use shows and takes input');

  // A subgraph use has no node folder, so its Inspector has no Code tab (6.4).
  [...document.querySelectorAll<HTMLButtonElement>('.graph-path button')].find((b) => b.textContent === 'main')!.click();
  const useNode = () => document.querySelector(`.react-flow__node[data-id="${use}"]`);
  await until('the use in the main graph after the lamp', BUILD_MS, () => document.querySelector('.graph-path') === null && !!useNode());
  rightClick(useNode()!.querySelector('.title') ?? useNode()!);
  await until('the use in the Inspector', RUN_MS, () => inspectorTitle().includes('fire_check') && inspectorTabs().length > 0);
  closeMenu();
  if (inspectorTabs().map((t) => t.textContent).join() !== 'Parameters') throw new Error(`a subgraph use has the tabs ${inspectorTabs().map((t) => t.textContent).join()}`);
  steps.push('a subgraph use has no Code tab');

  // The Shape list of the Look section gives each use of the subgraph a symbol, with undo (4.3, 6.4).
  const shape = () => document.querySelector<HTMLSelectElement>(`${inspector} select:has(option[value="chip"])`);
  const chip = () => useNode()?.querySelector('.patina-node.symbol') ?? null;
  const openUse = async (what: string) => {
    useNode()!.querySelector<HTMLButtonElement>('.open-subgraph')!.click();
    await until(what, BUILD_MS, () => document.querySelector('.graph-path [aria-current="page"]')?.textContent === '/ fire_check');
  };
  const toMain = async (what: string) => {
    [...document.querySelectorAll<HTMLButtonElement>('.graph-path button')].find((b) => b.textContent === 'main')!.click();
    await until(what, BUILD_MS, () => document.querySelector('.graph-path') === null && !!useNode());
  };
  await openUse('the subgraph for its Shape');
  await until('the Shape list of the subgraph', RUN_MS, () => !!shape());
  shape()!.value = 'chip';
  shape()!.dispatchEvent(new Event('change', { bubbles: true }));
  await until('the chip preview of the public node', RUN_MS, () => shape()?.value === 'chip' && !!document.querySelector(`${inspector} .symbol-preview`));
  await toMain('the main graph with the chip');
  await until('the use as a chip', BUILD_MS, () => !!chip());
  // Open subgraph in the node menu opens the subgraph, as the button on the use does.
  rightClick(useNode()!.querySelector('.title') ?? useNode()!);
  await until('the Open subgraph entry', RUN_MS, () => !!entry('Node', 'Open subgraph'));
  entry('Node', 'Open subgraph')!.click();
  await until('the subgraph from the node menu', BUILD_MS, () => document.querySelector('.graph-path [aria-current="page"]')?.textContent === '/ fire_check');
  v = await step('undo the Shape of the subgraph', () => api.undo());
  if (v.graph.symbol) throw new Error(`the undo kept the symbol ${JSON.stringify(v.graph.symbol)}`);
  await toMain('the main graph after the undo of the Shape');
  await until('the use as a box again', BUILD_MS, () => !chip());
  steps.push('give the uses of a subgraph a symbol from the Shape list');

  // Select all and Collapse into subgraph of the Inspector put the nodes into a new subgraph, with undo (6.1).
  const count = () => document.querySelectorAll('.react-flow__node').length;
  const all = count();
  rightClick(document.querySelector('.react-flow__pane')!);
  await until('the canvas menu in the main graph', RUN_MS, () => !!entry('Canvas', 'Select all'));
  entry('Canvas', 'Select all')!.click();
  await until('the subgraph tools of the selection', RUN_MS, () => inspectorTitle() === `${all} items selected`);
  [...document.querySelectorAll<HTMLButtonElement>(`${inspector} .btn`)].find((b) => b.textContent === 'Collapse into subgraph...')!.click();
  const nameField = () => document.querySelector<HTMLInputElement>('[aria-label="Subgraph name"]');
  await until('the subgraph name field', RUN_MS, () => !!nameField());
  enterText(nameField()!, 'all_check');
  await until('one use for all nodes', BUILD_MS, () => count() === 1 && document.querySelector('.react-flow__node .title')?.textContent?.includes('all_check') === true);
  // Expand subgraph in the menu of the use puts the nodes back in the main graph (6.1).
  const allUse = document.querySelector('.react-flow__node')!;
  rightClick(allUse.querySelector('.title') ?? allUse);
  await until('the Expand subgraph entry', RUN_MS, () => !!entry('Node', 'Expand subgraph'));
  entry('Node', 'Expand subgraph')!.click();
  await until('all nodes after Expand subgraph', BUILD_MS, () => count() === all);
  v = await step('undo the expand from the node menu', () => api.undo());
  await until('one use after the undo of the expand', BUILD_MS, () => count() === 1);
  v = await step('undo the collapse from the Inspector', () => api.undo());
  await until('all nodes after the undo of the collapse', BUILD_MS, () => count() === all);
  steps.push('collapse a selection from the Inspector, and expand it from the node menu');
  // Ctrl+G collapses the selection, and Ctrl+Shift+G expands the selected use (6.1).
  const pane = document.querySelector('.react-flow__pane')!;
  const ctrl = (key: string, shiftKey = false) => pane.dispatchEvent(new KeyboardEvent('keydown', { key, ctrlKey: true, shiftKey, bubbles: true, cancelable: true }));
  const selected = () => document.querySelectorAll('.react-flow__node.selected').length;
  ctrl('a');
  await until('all nodes selected for Ctrl+G', RUN_MS, () => selected() === all);
  ctrl('g');
  await until('the subgraph name field of Ctrl+G', RUN_MS, () => !!nameField());
  enterText(nameField()!, 'keys_check');
  await until('one use after Ctrl+G', BUILD_MS, () => count() === 1 && document.querySelector('.react-flow__node .title')?.textContent?.includes('keys_check') === true);
  ctrl('a');
  await until('the use selected for Ctrl+Shift+G', RUN_MS, () => selected() === 1);
  ctrl('G', true);
  await until('all nodes after Ctrl+Shift+G', BUILD_MS, () => count() === all);
  v = await step('undo the expand of Ctrl+Shift+G', () => api.undo());
  await until('one use after the undo of Ctrl+Shift+G', BUILD_MS, () => count() === 1);
  v = await step('undo the collapse of Ctrl+G', () => api.undo());
  await until('all nodes after the undo of Ctrl+G', BUILD_MS, () => count() === all);
  steps.push('collapse with Ctrl+G and expand with Ctrl+Shift+G');
  // Expand subgraphs in the node menu of two selected uses expands both (6.1).
  const titled = (name: string) => [...document.querySelectorAll('.react-flow__node .title')].filter((t) => t.textContent?.includes(name)).length;
  ctrl('a');
  await until('all nodes selected for two uses', RUN_MS, () => selected() === all);
  ctrl('g');
  await until('the subgraph name field for two uses', RUN_MS, () => !!nameField());
  enterText(nameField()!, 'two_check');
  await until('one use of two_check', BUILD_MS, () => count() === 1 && titled('two_check') === 1);
  ctrl('a');
  await until('the use selected for the copy', RUN_MS, () => selected() === 1);
  const useId = document.querySelector('.react-flow__node.selected')!.getAttribute('data-id')!;
  ctrl('c');
  let clip = '';
  await until('the clip of the use', RUN_MS, () => {
    clipboard.read().then((text) => (clip = text));
    return clip.includes(`"${useId}"`);
  });
  ctrl('v');
  await until('two uses of two_check', BUILD_MS, () => titled('two_check') === 2);
  ctrl('a');
  await until('the two uses selected', RUN_MS, () => selected() === 2);
  const twoUse = document.querySelector('.react-flow__node')!;
  rightClick(twoUse.querySelector('.title') ?? twoUse);
  await until('the Expand subgraphs entry', RUN_MS, () => !!entry('Node', 'Expand subgraphs'));
  entry('Node', 'Expand subgraphs')!.click();
  await until('the nodes of two uses after Expand subgraphs', BUILD_MS, () => titled('two_check') === 0 && count() === 2 * all);
  for (const what of ['expand', 'paste', 'collapse']) v = await step(`undo the ${what} of two uses`, () => api.undo());
  await until('all nodes after the undo of two uses', BUILD_MS, () => count() === all && titled('two_check') === 0);
  steps.push('Expand subgraphs in the node menu expands two uses');
  // Collapse into subgraph in the node menu of a selection opens the same name dialog.
  ctrl('a');
  await until('all nodes selected for the node menu', RUN_MS, () => selected() === all);
  const first = document.querySelector('.react-flow__node')!;
  rightClick(first.querySelector('.title') ?? first);
  await until('the Collapse into subgraph entry', RUN_MS, () => !!entry('Node', 'Collapse into subgraph...'));
  entry('Node', 'Collapse into subgraph...')!.click();
  await until('the subgraph name field of the node menu', RUN_MS, () => !!nameField());
  enterText(nameField()!, 'menu_check');
  await until('one use after the collapse of the node menu', BUILD_MS, () => count() === 1 && document.querySelector('.react-flow__node .title')?.textContent?.includes('menu_check') === true);
  // A use in a subgraph adds a crumb to the path bar. A click on the middle crumb opens its graph (15.9).
  const openOf = (name: string) =>
    [...document.querySelectorAll<HTMLElement>('.react-flow__node')].find((n) => n.querySelector('.title')?.textContent?.includes(name))?.querySelector<HTMLButtonElement>('.open-subgraph');
  const current = () => document.querySelector('.graph-path [aria-current="page"]')?.textContent;
  const crumb = (label: string) => [...document.querySelectorAll<HTMLButtonElement>('.graph-path button')].find((b) => b.textContent === label);
  openOf('menu_check')!.click();
  await until('the fire_check use in the menu_check subgraph', BUILD_MS, () => current() === '/ menu_check' && !!openOf('fire_check'));
  openOf('fire_check')!.click();
  await until('the menu_check crumb before fire_check', BUILD_MS, () => current() === '/ fire_check' && !!crumb('menu_check'));
  crumb('menu_check')!.click();
  await until('the menu_check subgraph from its crumb', BUILD_MS, () => current() === '/ menu_check');
  crumb('main')!.click();
  await until('the main graph after the nested use', BUILD_MS, () => document.querySelector('.graph-path') === null && count() === 1);
  steps.push('open a use in a subgraph and go back with the middle crumb of the path bar');
  v = await step('undo the collapse of the node menu', () => api.undo());
  await until('all nodes after the undo of the node menu collapse', BUILD_MS, () => count() === all);
  steps.push('collapse a selection from the node menu');
}

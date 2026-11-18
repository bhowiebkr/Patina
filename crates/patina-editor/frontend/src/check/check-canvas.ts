// The canvas steps of the self-check: a drop of a node folder from the Project panel, and the
// right-click menus of the empty canvas, a node and a connection, and copy and paste (15.2, 15.4, 15.11).

import { api, type View } from '../api';
import { RUN_MS, type CheckContext } from './check-fixtures';
import { clipboard } from '../canvas/clip';
import { NODE_MIME } from '../canvas/useAddNode';

const count = (sel: string) => document.querySelectorAll(sel).length;
const NODES = '.react-flow__node';
/** The entry of an open menu, found by its label without the shortcut text. */
const entry = (menu: string, label: string) =>
  [...document.querySelectorAll<HTMLButtonElement>(`[role="menu"][aria-label="${menu}"] [role="menuitem"]`)].find((b) => b.childNodes[0]?.textContent === label);

function rightClick(el: Element, x?: number, y?: number) {
  const r = el.getBoundingClientRect();
  const at = { bubbles: true, cancelable: true, button: 2, clientX: x ?? r.left + r.width / 2, clientY: y ?? r.top + r.height / 2 };
  el.dispatchEvent(new MouseEvent('contextmenu', at));
}

/** Runs the canvas steps on the open main graph, and returns the last view. */
export async function checkCanvas({ step, until, steps }: CheckContext, v: View): Promise<View> {
  // A drop of a node folder adds a use at the drop point (15.4).
  const pane = document.querySelector('.react-flow__pane')!;
  const r = pane.getBoundingClientRect();
  const folder = v.nodes.find((n) => !n.subgraph && !n.builtin)!.name;
  const data = new DataTransfer();
  data.setData(NODE_MIME, folder);
  const drag = { bubbles: true, cancelable: true, dataTransfer: data, clientX: r.left + 40, clientY: r.top + 40 };
  const nodes = count(NODES);
  pane.dispatchEvent(new DragEvent('dragover', drag));
  pane.dispatchEvent(new DragEvent('drop', drag));
  await until('the dropped node', RUN_MS, () => count(NODES) === nodes + 1);
  v = await step('undo the drop', () => api.undo());
  await until('the graph after the undo of the drop', RUN_MS, () => count(NODES) === nodes);
  steps.push(`node folder ${folder} dropped on the canvas`);

  // The menu of the empty canvas.
  rightClick(pane, r.left + 40, r.top + 40);
  await until('the canvas menu', RUN_MS, () => !!entry('Canvas', 'Fit view'));
  for (const label of ['Add node...', 'Select all', 'Arrange']) {
    if (!entry('Canvas', label)) throw new Error(`the canvas menu has no ${label}`);
  }
  entry('Canvas', 'Select all')!.click();
  await until('all nodes selected', RUN_MS, () => [...document.querySelectorAll(NODES)].every((n) => n.classList.contains('selected')));
  document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
  await until('the clear selection', RUN_MS, () => count(`${NODES}.selected`) === 0);

  // The menu of a node: a right-click selects the node, and Delete node asks, then removes it with undo.
  const node = document.querySelector(`${NODES}-patina`)!;
  rightClick(node.querySelector('.title') ?? node);
  await until('the node menu', RUN_MS, () => !!entry('Node', 'Delete node'));
  if (!node.classList.contains('selected')) throw new Error('a right-click did not select the node');
  entry('Node', 'Delete node')!.click();
  // The menu asks first (15.2).
  const answer = (text: string) => [...document.querySelectorAll<HTMLButtonElement>('.dialog button')].find((b) => b.textContent === text);
  await until('the delete question', RUN_MS, () => !!answer('Delete'));
  answer('Delete')!.click();
  await until('the deleted node', RUN_MS, () => count(NODES) === nodes - 1);
  v = await step('undo the delete from the node menu', () => api.undo());
  await until('the node after the undo', RUN_MS, () => count(NODES) === nodes);

  // Copy of the node menu and Paste of the canvas menu. The self-check keeps the clip in memory.
  // The undo made a new element for the node.
  const copied = document.querySelector(`${NODES}-patina`)!;
  rightClick(copied.querySelector('.title') ?? copied);
  await until('the Copy entry', RUN_MS, () => !!entry('Node', 'Copy'));
  entry('Node', 'Copy')!.click();
  let clip = '';
  await until('the copied clip', RUN_MS, () => {
    clipboard.read().then((text) => (clip = text));
    return clip.startsWith('patina-clip');
  });
  rightClick(pane, r.left + 40, r.top + 40);
  await until('the Paste entry', RUN_MS, () => !!entry('Canvas', 'Paste'));
  entry('Canvas', 'Paste')!.click();
  const selected = () => [...document.querySelectorAll(`${NODES}.selected`)].map((n) => n.getAttribute('data-id'));
  await until('the pasted node, selected', RUN_MS, () => count(NODES) === nodes + 1 && selected().length === 1 && selected()[0] !== copied.getAttribute('data-id'));
  // Ctrl+C, Ctrl+X and Ctrl+V over the canvas copy and cut the pasted node, and paste it again.
  const press = (key: string) => pane.dispatchEvent(new KeyboardEvent('keydown', { key, ctrlKey: true, bubbles: true }));
  const id = selected()[0]!;
  press('c');
  await until('the clip of Ctrl+C', RUN_MS, () => {
    clipboard.read().then((text) => (clip = text));
    return clip.includes(`"${id}"`);
  });
  press('x');
  await until('the node cut with Ctrl+X', RUN_MS, () => count(NODES) === nodes);
  press('v');
  await until('the node of Ctrl+V, selected', RUN_MS, () => count(NODES) === nodes + 1 && selected().length === 1);
  for (const what of ['the paste of Ctrl+V', 'the cut', 'the paste']) v = await step(`undo ${what}`, () => api.undo());
  await until('the graph after the undo of the paste', RUN_MS, () => count(NODES) === nodes);
  steps.push('copy, cut and paste of a node, from the menus and the keys');

  // The menu of a connection.
  const wires = count('.wire-hit');
  rightClick(document.querySelector('.wire-hit')!);
  await until('the connection menu', RUN_MS, () => !!entry('Connection', 'Delete connection'));
  entry('Connection', 'Delete connection')!.click();
  await until('the deleted connection', RUN_MS, () => count('.wire-hit') === wires - 1);
  v = await step('undo the delete of the connection', () => api.undo());
  await until('the connection after the undo', RUN_MS, () => count('.wire-hit') === wires);
  document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
  steps.push('right-click menus of the canvas, a node and a connection');
  return v;
}

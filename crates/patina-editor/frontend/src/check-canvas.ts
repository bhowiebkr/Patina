// The canvas steps of the self-check: a drop of a node folder from the Project panel, and the
// right-click menus of the empty canvas, a node and a connection (15.2, 15.4).

import { api, type View } from './api';
import { RUN_MS } from './check-fixtures';
import { NODE_MIME } from './GraphView';

type Step = (name: string, job: () => Promise<View>) => Promise<View>;
type Until = (what: string, ms: number, test: () => boolean) => Promise<void>;

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
export async function checkCanvas(step: Step, until: Until, steps: string[], v: View): Promise<View> {
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

  // The menu of a node: a right-click selects the node, and Delete node removes it with undo.
  const node = document.querySelector(`${NODES}-patina`)!;
  rightClick(node.querySelector('.title') ?? node);
  await until('the node menu', RUN_MS, () => !!entry('Node', 'Delete node'));
  if (!node.classList.contains('selected')) throw new Error('a right-click did not select the node');
  entry('Node', 'Delete node')!.click();
  await until('the deleted node', RUN_MS, () => count(NODES) === nodes - 1);
  v = await step('undo the delete from the node menu', () => api.undo());
  await until('the node after the undo', RUN_MS, () => count(NODES) === nodes);

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

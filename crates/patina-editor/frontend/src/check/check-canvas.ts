// The canvas steps of the self-check: a drop of a node folder from the Project panel, and the
// right-click menus of the empty canvas, a node and a connection, and copy and paste (15.2, 15.4, 15.11).

import { api, type View } from '../api';
import { RUN_MS, button, entry, leftTab, rightClick, statusText, topButton, type CheckContext } from './check-fixtures';
import { clipboard } from '../canvas/clip';
import { LIBRARY_MIME, NODE_MIME } from '../canvas/useAddNode';

const count = (sel: string) => document.querySelectorAll(sel).length;
const NODES = '.react-flow__node';
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

  // The Library panel shows the preview of a library node with what it brings. A drag and Add to graph add it (15.2).
  leftTab('Library').click();
  const libItem = () => [...document.querySelectorAll<HTMLButtonElement>('[aria-label=Library] .item')].find((b) => b.textContent === 'number');
  await until('the number node of the library in the Library panel', RUN_MS, () => !!libItem());
  document.querySelector<HTMLButtonElement>('[aria-label="Read the libraries again"]')!.click();
  await until('the library after a second read', RUN_MS, () => !!libItem());
  libItem()!.click();
  const brings = () => [...document.querySelectorAll('[aria-label="Brings"] li')].map((li) => li.textContent ?? '');
  await until('the Brings list of the preview', RUN_MS, () => brings().some((t) => t.startsWith('nodes/number '))).catch((e: Error) => {
    throw new Error(`${e.message} (rows ${brings().join(' | ')})`);
  });
  const libData = new DataTransfer();
  libItem()!.dispatchEvent(new DragEvent('dragstart', { bubbles: true, cancelable: true, dataTransfer: libData }));
  if (!libData.getData(LIBRARY_MIME)) throw new Error('a drag of a Library item gives no data');
  const libDrag = { ...drag, dataTransfer: libData };
  pane.dispatchEvent(new DragEvent('dragover', libDrag));
  pane.dispatchEvent(new DragEvent('drop', libDrag));
  await until('the library node of the drop', RUN_MS, () => count(NODES) === nodes + 1);
  v = await step('undo the library drop', () => api.undo());
  await until('the graph after the undo of the library drop', RUN_MS, () => count(NODES) === nodes);
  await until('Add to graph', RUN_MS, () => button('Add to graph')?.disabled === false);
  button('Add to graph')!.click();
  await until('the library node of Add to graph', RUN_MS, () => count(NODES) === nodes + 1);
  v = await step('undo Add to graph', () => api.undo());
  await until('the graph after the undo of Add to graph', RUN_MS, () => count(NODES) === nodes);
  leftTab('Project').click();
  steps.push('Library panel: preview with Brings, drop and Add to graph');

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
  // Ctrl+A over the canvas selects all nodes too.
  pane.dispatchEvent(new KeyboardEvent('keydown', { key: 'a', ctrlKey: true, bubbles: true }));
  await until('all nodes selected with Ctrl+A', RUN_MS, () => [...document.querySelectorAll(NODES)].every((n) => n.classList.contains('selected')));
  document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
  await until('the clear selection after Ctrl+A', RUN_MS, () => count(`${NODES}.selected`) === 0);

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
  // Fit node of the node menu puts the node in the middle of the canvas.
  const fitted = document.querySelector(`${NODES}-patina`)!;
  // The node of the undo grows when its GUI comes, so Fit node waits for a stable size.
  let [size, since] = ['', Date.now()];
  await until('a stable size of the node after the undo', RUN_MS, () => {
    const b = fitted.getBoundingClientRect();
    if (`${b.width} ${b.height}` !== size) [size, since] = [`${b.width} ${b.height}`, Date.now()];
    return Date.now() - since > 1000;
  });
  const middle = (e: Element) => {
    const b = e.getBoundingClientRect();
    return [b.left + b.width / 2, b.top + b.height / 2];
  };
  const centered = () => {
    const [nx, ny] = middle(fitted);
    const [px, py] = middle(pane);
    return Math.abs(nx - px) < 2 && Math.abs(ny - py) < 2;
  };
  const fitNode = async () => {
    rightClick(fitted.querySelector('.title') ?? fitted);
    await until('the Fit node entry', RUN_MS, () => !!entry('Node', 'Fit node'));
    entry('Node', 'Fit node')!.click();
    await until('the node in the middle after Fit node', RUN_MS, centered).catch((e: Error) => {
      const [b, p] = [fitted, pane].map((el) => el.getBoundingClientRect()).map((r) => [r.left, r.top, r.width, r.height].map(Math.round).join(' '));
      throw new Error(`${e.message} (node ${fitted.getAttribute('data-id')} ${b}, in page ${fitted.isConnected}, pane ${p}, status ${statusText()})`);
    });
  };
  const inside = (b: DOMRect, p = pane.getBoundingClientRect()) => b.left >= p.left && b.right <= p.right && b.top >= p.top && b.bottom <= p.bottom;
  const allFitted = () => !centered() && [...document.querySelectorAll(NODES)].every((n) => inside(n.getBoundingClientRect()));
  await fitNode();
  // Fit of the top bar fits all nodes, also when a node is selected.
  topButton('Fit')!.click();
  await until('all nodes in the canvas after Fit of the top bar', RUN_MS, allFitted);
  // Esc clears the selection, and the F key fits all nodes again for the next steps.
  await fitNode();
  document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
  await until('the clear selection after Fit node', RUN_MS, () => count(`${NODES}.selected`) === 0);
  document.dispatchEvent(new KeyboardEvent('keydown', { key: 'f', bubbles: true }));
  await until('all nodes in the canvas after the F key', RUN_MS, allFitted);
  steps.push('Fit node in the node menu, Fit in the top bar, and the F key for all nodes');

  // A right-click in a selection keeps it. Fit selection fits the selected nodes, and Delete selection asks first.
  await fitNode();
  pane.dispatchEvent(new KeyboardEvent('keydown', { key: 'a', ctrlKey: true, bubbles: true }));
  const all = () => count(`${NODES}.selected`) === nodes;
  await until('all nodes selected for the selection menu', RUN_MS, all);
  const selectionMenu = async (label: string) => {
    rightClick(fitted.querySelector('.title') ?? fitted);
    await until(`the ${label} entry`, RUN_MS, () => !!entry('Node', label));
    if (!all()) throw new Error('a right-click in the selection changed the selection');
    entry('Node', label)!.click();
  };
  await selectionMenu('Fit selection');
  await until('all nodes in the canvas after Fit selection', RUN_MS, allFitted);
  await selectionMenu('Delete selection');
  await until('the delete question for the selection', RUN_MS, () => !!answer('Delete'));
  answer('Delete')!.click();
  await until('no nodes after Delete selection', RUN_MS, () => count(NODES) === 0);
  v = await step('undo the delete of the selection', () => api.undo());
  await until('all nodes after the undo of Delete selection', RUN_MS, () => count(NODES) === nodes);
  steps.push('Fit selection and Delete selection in the node menu of a selection');

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
  // Cut of the node menu removes the node, as Ctrl+X does.
  const pasted = document.querySelector(`${NODES}.selected`)!;
  rightClick(pasted.querySelector('.title') ?? pasted);
  await until('the Cut entry', RUN_MS, () => !!entry('Node', 'Cut'));
  entry('Node', 'Cut')!.click();
  await until('the node cut from the menu', RUN_MS, () => count(NODES) === nodes);
  for (const what of ['the cut from the menu', 'the paste of Ctrl+V', 'the cut', 'the paste']) v = await step(`undo ${what}`, () => api.undo());
  await until('the graph after the undo of the paste', RUN_MS, () => count(NODES) === nodes);
  steps.push('copy, cut and paste of a node, from the menus and the keys');

  // Shift+L and Arrange of the canvas menu put a far node back in the layout, with undo (15.9).
  const far = document.querySelector(`${NODES}-patina`)!.getAttribute('data-id')!;
  const top = () => document.querySelector(`${NODES}[data-id="${far}"]`)!.getBoundingClientRect().top;
  const [fx, fy] = v.graph.node!.find((n) => n.id === far)!.pos!;
  const start = top();
  v = await step('move a node far down', () => api.edit({ kind: 'MoveNode', id: far, pos: [fx + 300, fy + 1200] }));
  await until('the node far down', RUN_MS, () => top() - start > 100);
  const farTop = top();
  document.dispatchEvent(new KeyboardEvent('keydown', { key: 'L', shiftKey: true, bubbles: true }));
  await until('the node in the layout after Shift+L', RUN_MS, () => farTop - top() > 100);
  v = await step('undo the arrange of Shift+L', () => api.undo());
  await until('the node far down after the undo', RUN_MS, () => Math.abs(top() - farTop) < 1);
  rightClick(pane, r.left + 40, r.top + 40);
  await until('the Arrange entry', RUN_MS, () => !!entry('Canvas', 'Arrange'));
  entry('Canvas', 'Arrange')!.click();
  await until('the node in the layout after Arrange', RUN_MS, () => farTop - top() > 100);
  for (const what of ['the arrange of the menu', 'the move']) v = await step(`undo ${what}`, () => api.undo());
  await until('the node at its start after the undo', RUN_MS, () => Math.abs(top() - start) < 1);
  steps.push('Shift+L and Arrange of the canvas menu put a far node back in the layout');

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

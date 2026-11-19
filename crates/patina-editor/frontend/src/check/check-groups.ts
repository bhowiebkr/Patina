// The group and note steps of the self-check: Add group, a color and a title from the group menu,
// Shift+G around a node, a drag of a group that carries the node, and a Markdown note (15.12).

import { api, type View } from '../api';
import { GROUP_RGB } from '../canvas/groups';
import { RUN_MS, closeMenu, enterText, entry, rightClick, type CheckContext } from './check-fixtures';

const GROUPS = '.react-flow__node-groupBox';
const NOTES = '.react-flow__node-note';
const all = (sel: string) => [...document.querySelectorAll<HTMLElement>(sel)];
const ids = (sel: string) => new Set(all(sel).map((e) => e.getAttribute('data-id')));
/** The element that was not in `before`. */
const added = (sel: string, before: Set<string | null>) => all(sel).find((e) => !before.has(e.getAttribute('data-id')));
const escape = () => document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
const color = (label: string) =>
  [...document.querySelectorAll<HTMLButtonElement>('[role="menu"][aria-label="Group"] [role="menuitemcheckbox"]')].find((b) => b.childNodes[0]?.textContent === label);

/** Runs the group and note steps on the open main graph, and returns the last view. */
export async function checkGroups({ step, until, steps }: CheckContext, v: View): Promise<View> {
  const pane = document.querySelector('.react-flow__pane')!;
  const r = pane.getBoundingClientRect();
  const groups = v.graph.group?.length ?? 0;
  const notes = v.graph.note?.length ?? 0;

  // Add group of the canvas menu adds an amber group. Its menu sets the color, and a double-click sets the title.
  const beforeGroups = ids(GROUPS);
  rightClick(pane, r.left + 40, r.top + 40);
  await until('the Add group entry', RUN_MS, () => !!entry('Canvas', 'Add group'));
  entry('Canvas', 'Add group')!.click();
  await until('the new group', RUN_MS, () => !!added(GROUPS, beforeGroups));
  const group = added(GROUPS, beforeGroups)!;
  const box = group.querySelector<HTMLElement>('.group-box')!;
  const title = group.querySelector<HTMLElement>('.group-title')!;
  if (box.style.getPropertyValue('--group') !== GROUP_RGB.amber) throw new Error(`a new group is not amber: ${box.style.getPropertyValue('--group')}`);
  rightClick(title);
  await until('the group menu', RUN_MS, () => !!color('Teal'));
  if (color('Amber')!.getAttribute('aria-checked') !== 'true') throw new Error('the group menu does not mark the color of the group');
  color('Teal')!.click();
  await until('the teal group', RUN_MS, () => box.style.getPropertyValue('--group') === GROUP_RGB.teal);
  title.dispatchEvent(new MouseEvent('dblclick', { bubbles: true, cancelable: true, view: window }));
  await until('the title field of the group', RUN_MS, () => !!title.querySelector('input.item-field'));
  enterText(title.querySelector<HTMLInputElement>('input.item-field')!, 'Filters');
  await until('the new title of the group', RUN_MS, () => title.textContent === 'Filters');
  v = await step('undo the group title', () => api.undo());
  v = await step('undo the group color', () => api.undo());
  v = await step('undo Add group', () => api.undo());
  if ((v.graph.group?.length ?? 0) !== groups) throw new Error(`three undos did not remove the group: ${JSON.stringify(v.graph.group)}`);
  await until('no new group after the undos', RUN_MS, () => !added(GROUPS, beforeGroups));

  // Shift+G puts a group around the selected node. A drag of the group title moves the node too, in one edit.
  const node = all('.react-flow__node-patina')[0];
  const nodeId = node.getAttribute('data-id')!;
  const pos = (view: View) => JSON.stringify(view.graph.node?.find((n) => n.id === nodeId)?.pos);
  const start = pos(v);
  rightClick(node.querySelector('.title') ?? node);
  await until('the node menu before Shift+G', RUN_MS, () => !!entry('Node', 'Group selection'));
  closeMenu();
  await until('no menu before Shift+G', RUN_MS, () => !document.querySelector('.menu-layer'));
  document.dispatchEvent(new KeyboardEvent('keydown', { key: 'G', shiftKey: true, bubbles: true }));
  await until('the group around the node', RUN_MS, () => !!added(GROUPS, beforeGroups));
  const around = added(GROUPS, beforeGroups)!;
  const [g, n] = [around.getBoundingClientRect(), node.getBoundingClientRect()];
  if (n.left < g.left || n.top < g.top || n.right > g.right || n.bottom > g.bottom) throw new Error('the group of Shift+G does not contain the node');
  escape();
  const grip = around.querySelector('.group-title')!;
  const b = grip.getBoundingClientRect();
  const mouse = (dy: number) => ({ bubbles: true, cancelable: true, view: window, button: 0, clientX: b.left + 10, clientY: b.top + b.height / 2 + dy });
  const top = node.getBoundingClientRect().top;
  grip.dispatchEvent(new MouseEvent('mousedown', mouse(0)));
  for (const dy of [10, 40, 80]) document.dispatchEvent(new MouseEvent('mousemove', mouse(dy)));
  document.dispatchEvent(new MouseEvent('mouseup', mouse(80)));
  await until('the node carried by the group', RUN_MS, () => node.getBoundingClientRect().top - top > 40 && !around.classList.contains('dragging'));
  // shortcut: a short wait lets the move edit arrive before the undo, change it when the canvas shows the edit count.
  await new Promise((done) => setTimeout(done, 400));
  v = await step('undo the drag of the group', () => api.undo());
  if (pos(v) !== start || (v.graph.group?.length ?? 0) !== groups + 1) throw new Error(`one undo did not move the group and its node back: ${pos(v)} ${JSON.stringify(v.graph.group)}`);
  v = await step('undo Shift+G', () => api.undo());
  await until('no group after the undo of Shift+G', RUN_MS, () => !added(GROUPS, beforeGroups));

  // Add note opens the text field. Ctrl+Enter keeps the text, and the note shows it as Markdown.
  const beforeNotes = ids(NOTES);
  rightClick(pane, r.left + 40, r.top + 40);
  await until('the Add note entry', RUN_MS, () => !!entry('Canvas', 'Add note'));
  entry('Canvas', 'Add note')!.click();
  await until('the text field of the new note', RUN_MS, () => !!added(NOTES, beforeNotes)?.querySelector('textarea.item-field'));
  const note = added(NOTES, beforeNotes)!;
  const field = note.querySelector<HTMLTextAreaElement>('textarea.item-field')!;
  Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value')!.set!.call(field, '# Plan\n\n- one\n- two');
  field.dispatchEvent(new Event('input', { bubbles: true }));
  field.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', ctrlKey: true, bubbles: true, cancelable: true }));
  await until('the Markdown of the note', RUN_MS, () => note.querySelector('.md h1')?.textContent === 'Plan' && note.querySelectorAll('.md li').length === 2);
  v = await step('undo the note text', () => api.undo());
  v = await step('undo Add note', () => api.undo());
  if ((v.graph.note?.length ?? 0) !== notes) throw new Error(`two undos did not remove the note: ${JSON.stringify(v.graph.note)}`);
  await until('no new note after the undos', RUN_MS, () => !added(NOTES, beforeNotes));
  steps.push('a group got a color and a title from its menu, Shift+G grouped a node, a group drag carried it, and a note showed Markdown');
  return v;
}

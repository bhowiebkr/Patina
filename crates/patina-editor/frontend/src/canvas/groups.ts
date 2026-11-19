// The groups and the notes of the canvas (spec 15.12). They are visual only: the build does not read them.

import type { Node } from '@xyflow/react';
import type { Edit, Group, GroupColor, Note, Point } from '../api';

export const GROUP_MIN: Point = [120, 60];
export const NOTE_MIN = 80;
export const NOTE_MAX = 2000;
export const NOTE_WIDTH = 240;
export const MAX_TITLE = 64;
export const MAX_TEXT = 20000;
/** The height of the title bar of a group. */
export const GROUP_TITLE = 24;
/** The space between the items and the edge of a new group around them. */
export const GROUP_MARGIN = 24;
export const NEW_GROUP: Point = [320, 200];
/** The z-index of the largest group. A selected node gets 1000 more, so a group stays behind the nodes. */
export const GROUP_Z = -2000;

/** The RGB of each group color, for rgba() in the CSS. Amber is the accent, and the others are lane colors. */
export const GROUP_RGB: Record<GroupColor, string> = {
  gray: '238, 238, 235',
  amber: '223, 166, 75',
  green: '139, 214, 148',
  teal: '121, 212, 200',
  violet: '185, 155, 223',
  pink: '222, 143, 186',
  copper: '217, 161, 122',
};

export type GroupBoxNode = Node<{ group: Group; editable: boolean }, 'groupBox'>;
export type NoteNode = Node<{ note: Note; editable: boolean }, 'note'>;
export type ItemNode = GroupBoxNode | NoteNode;

export type Box = { l: number; t: number; r: number; b: number };

/** The kind and the file ID of a group or a note, from its canvas ID, or null for a node. */
export function itemOf(id: string): { kind: 'group' | 'note'; id: string } | null {
  if (id.startsWith('group:')) return { kind: 'group', id: id.slice(6) };
  if (id.startsWith('note:')) return { kind: 'note', id: id.slice(5) };
  return null;
}

/** True when the full box `a` is in the box `b`. */
export function inside(a: Box, b: Box): boolean {
  return a.l >= b.l && a.t >= b.t && a.r <= b.r && a.b <= b.b;
}

/** The IDs of the items whose full box is in one of the groups. */
export function itemsIn(groups: readonly Box[], items: readonly (Box & { id: string })[]): string[] {
  return items.filter((i) => groups.some((g) => inside(i, g))).map((i) => i.id);
}

/** A new group around the boxes, with a margin and room for the title bar. */
export function groupAround(boxes: readonly Box[]): { pos: Point; size: Point } {
  const l = Math.min(...boxes.map((b) => b.l)) - GROUP_MARGIN;
  const t = Math.min(...boxes.map((b) => b.t)) - GROUP_MARGIN - GROUP_TITLE;
  const r = Math.max(...boxes.map((b) => b.r)) + GROUP_MARGIN;
  const b = Math.max(...boxes.map((x) => x.b)) + GROUP_MARGIN;
  return { pos: [Math.round(l), Math.round(t)], size: [Math.max(GROUP_MIN[0], Math.round(r - l)), Math.max(GROUP_MIN[1], Math.round(b - t))] };
}

/** The edit that sets a group or a note with its place and size in the view. The canvas maps them back to the graph. */
export function itemEdit(n: ItemNode, change: Partial<Group & Note> = {}): Edit {
  const round = (p: Point): Point => [Math.round(p[0]), Math.round(p[1])];
  const pos = round(change.pos ?? [n.position.x, n.position.y]);
  if (n.type === 'groupBox') {
    const g = n.data.group;
    const size = round(change.size ?? [n.width ?? g.size[0], n.height ?? g.size[1]]);
    return { kind: 'SetGroup', group: { ...g, title: change.title ?? g.title, color: change.color ?? g.color, pos, size } };
  }
  const note = n.data.note;
  return { kind: 'SetNote', note: { ...note, text: change.text ?? note.text, width: Math.round(change.width ?? note.width), pos } };
}

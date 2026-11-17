// The identity of each lane of a bus (spec 6.3, 15.9): the bus name, the lane names, the color
// family and the lane under the pointer, shared by the wires and the bus sockets of the nodes.

import { createContext } from 'react';
import type { KindInfo, Value } from '../api.ts';

/** The color families of the lanes (6.3). */
export const FAMILIES = ['green', 'teal', 'violet', 'pink', 'copper'];
/** A lit lane: neutral, then each family in turn. These are the focus colors of docs/gui-visual-guide.md. */
export const LANE_COLORS = [
  [238, 238, 235],
  [139, 214, 148],
  [121, 212, 200],
  [185, 155, 223],
  [222, 143, 186],
  [217, 161, 122],
];

/** The distance between the centers of two lanes of a ribbon, in graph pixels. */
export const PITCH = 5;

/** The kind of a type. An input type `&T` or `&mut T` has the kind of `T`. */
export function kindOf(kinds: readonly KindInfo[], ty: string | undefined): KindInfo | undefined {
  const bare = ty?.replace(/^&\s*(mut\s+)?/, '');
  return bare === undefined ? undefined : kinds.find((k) => k.ty === bare);
}

/** The color family of a kind: 0 for neutral, else 1 + its index in FAMILIES. */
export function familyOf(kind: KindInfo | undefined): number {
  return kind?.color ? FAMILIES.indexOf(kind.color) + 1 : 0;
}

/**
 * The lanes of a wire, the most significant first: the live lanes, else all off lanes when the
 * kind gives `lanes`, else one lane for a `bool`, else "" for one plain line.
 */
/** The width in px of a wire with these lanes. */
export const wireWidth = (bits: string) => Math.max(2, bits.length * PITCH);

export function wireBits(value: Value | undefined, kind: KindInfo | undefined): string {
  if (value?.lanes) return value.lanes.bits;
  if (kind?.lanes.length) return '0'.repeat(kind.lanes.length);
  return value?.text === 'true' ? '1' : value?.text === 'false' ? '0' : '';
}

/**
 * The text of one lane, for example "ctrl[15] HLT = 1". `names` are the lane names of the kind,
 * which a value with another number of lanes does not use. With no live value, the state is left out.
 */
export function laneTip(bus: string, names: readonly string[], bits: string, i: number, live: boolean): string {
  const bit = String(bits.length - 1 - i);
  const name = names.length === bits.length ? names[i] : bit;
  return `${bus}[${bit}]${name === bit ? '' : ` ${name}`}${live ? ` = ${bits[i]}` : ''}`;
}

/** The lane at the signed distance `d` from the center line of a ribbon of `n` lanes, or -1 when outside. */
export function laneAt(d: number, n: number): number {
  const half = (n * PITCH) / 2;
  if (n < 2 || Math.abs(d) > half + 2) return -1;
  return Math.min(n - 1, Math.max(0, Math.floor((d + half) / PITCH)));
}

/** The lane under the pointer: on a wire, or on a contact of a bus socket. `to` is null for all wires of `from`. */
export interface LaneHover {
  from: string;
  to: string | null;
  lane: number;
  /** The wire and the point in the graph of a hover on a wire, for its tooltip. */
  wire?: string;
  x?: number;
  y?: number;
}

export class HoverStore {
  private state: LaneHover | null = null;
  private readonly fns = new Set<() => void>();

  readonly subscribe = (fn: () => void) => {
    this.fns.add(fn);
    return () => void this.fns.delete(fn);
  };

  readonly get = () => this.state;

  set(state: LaneHover | null) {
    if (state === null && this.state === null) return;
    this.state = state;
    for (const fn of this.fns) fn();
  }
}

/** The hovered lane of the wire from `from` to `to`, or -1. `to` is null for an output, which all of its wires reach. */
export function hoveredLane(hover: LaneHover | null, from: string, to: string | null): number {
  return hover && from && hover.from === from && (hover.to === null || to === null || hover.to === to) ? hover.lane : -1;
}

/** A pin whose kind gives lanes (15.9): a socket with one contact for each lane. */
export interface Socket {
  /** The pin name, which the socket shows with the lane range. */
  bus: string;
  names: readonly string[];
  /** The live lanes, else all off. */
  bits: string;
  live: boolean;
  stale: boolean;
  family: number;
  /** The output of the lanes, or "" for an input with no wire, and the input or null for an output. */
  from: string;
  to: string | null;
}

/** The socket of a pin, or undefined when its kind gives no lanes. A value with another number of lanes is not shown. */
export function socketOf(kind: KindInfo | undefined, bus: string, value: Value | undefined, stale: boolean, from: string, to: string | null): Socket | undefined {
  const n = kind?.lanes.length ?? 0;
  if (!kind || n === 0) return undefined;
  const live = value?.lanes?.bits.length === n;
  return { bus, names: kind.lanes, bits: live ? value!.lanes!.bits : '0'.repeat(n), live, stale, family: familyOf(kind), from, to };
}

export const HoverContext = createContext<HoverStore>(new HoverStore());

// The labels above the wires, which hide when something covers them (spec 15.9).

import type { Dot } from './marks';
import type { Rect } from './route';
import type { Line } from './router';

interface Entry {
  id: string;
  label: Rect | null;
  badge: Rect | null;
  /** A lower rank shows first. A label with a rank below 0 always shows. */
  rank: number;
  hidden: boolean;
  badgeHidden: boolean;
}

const overlap = (a: Rect, b: Rect, m = 0) => a.l < b.r + m && b.l - m < a.r && a.t < b.b + m && b.t - m < a.b;
/** The pins stick out 6 px from a node body, with their shadow. */
const PIN = 6;
/** The rank of a type label, which hides with the badge of its wire. */
const TYPE = 2;
/** The smallest space between two shown labels, so that their texts do not run together. */
const SPACE = 4;

/** True when a segment of a wire other than `id`, and not of the output `net`, comes within `m` px of the box. */
function crossed(box: Rect, id: string, lines: ReadonlyMap<string, Line>, net?: string, m = 1): boolean {
  for (const [other, { points: p, width, net: n }] of lines) {
    if (other === id || n === net) continue;
    for (let k = 0; k + 1 < p.length; k++) {
      const [[x0, y0], [x1, y1]] = [p[k], p[k + 1]];
      if (overlap(box, { l: Math.min(x0, x1), t: Math.min(y0, y1), r: Math.max(x0, x1), b: Math.max(y0, y1) }, width / 2 + m)) return true;
    }
  }
  return false;
}

/**
 * The wire labels that hide. A label hides when a value badge, a node body, another wire or a label
 * of a lower rank covers it. The sockets and the pin tooltips still show the names.
 */
export class Labels {
  private entries = new Map<string, Entry>();
  private rects: readonly Rect[] = [];
  private dots: readonly Dot[] = [];
  private lines: ReadonlyMap<string, Line> = new Map();
  private order: Entry[] = [];
  private listeners = new Set<() => void>();
  private queued = false;

  /** The label box and the badge box of a wire, in flow coordinates. */
  set(id: string, label: Rect | null, badge: Rect | null, rank: number) {
    const e = this.entries.get(id);
    if (e) Object.assign(e, { label, badge, rank });
    else this.entries.set(id, { id, label, badge, rank, hidden: false, badgeHidden: false });
    this.queue();
  }

  remove(id: string) {
    if (this.entries.delete(id)) this.queue();
  }

  setObstacles(rects: readonly Rect[]) {
    this.rects = rects;
    this.queue();
  }

  /** The junction dots of the routed wires, the same array until they change (15.9). */
  setDots(dots: readonly Dot[]) {
    if (dots === this.dots) return;
    this.dots = dots;
    this.queue();
  }

  /** The paths of the routed wires, the same map until they change (15.9). */
  setLines(lines: ReadonlyMap<string, Line>) {
    if (lines === this.lines) return;
    this.lines = lines;
    this.queue();
  }

  hidden = (id: string): boolean => this.entries.get(id)?.hidden ?? false;

  badgeHidden = (id: string): boolean => this.entries.get(id)?.badgeHidden ?? false;

  subscribe = (f: () => void) => {
    this.listeners.add(f);
    return () => void this.listeners.delete(f);
  };

  private queue() {
    if (this.queued) return;
    this.queued = true;
    queueMicrotask(() => {
      this.queued = false;
      this.compute();
    });
  }

  /**
   * Shows the badges, then the labels, in the order of their rank. A badge hides when it covers a node
   * body, a pin, a junction dot or a wire of another output, or when a shown badge covers it.
   * A label hides when a shown box covers it, or when a shown label is nearer than SPACE.
   * A type label also hides with its badge, so it does not stand alone beside the badge of another wire.
   * A label also hides when another routed wire goes through it.
   */
  compute() {
    const order = this.order;
    order.length = 0;
    let changed = false;
    for (const e of this.entries.values()) order.push(e);
    order.sort((a, b) => a.rank - b.rank || (a.id < b.id ? -1 : a.id > b.id ? 1 : 0));
    const shown = (o: Entry) => o.badge !== null && !o.badgeHidden;
    for (let i = 0; i < order.length; i++) {
      const e = order[i];
      let hide = false;
      const badge = e.badge;
      if (badge && e.rank >= 0) {
        hide = this.rects.some((r) => overlap(badge, r, PIN)) || this.dots.some(([x, y, r]) => overlap(badge, { l: x, t: y, r: x, b: y }, r));
        hide ||= crossed(badge, e.id, this.lines, this.lines.get(e.id)?.net, 0);
        for (let j = 0; j < i && !hide; j++) hide = shown(order[j]) && overlap(badge, order[j].badge!);
      }
      if (e.badgeHidden !== hide) [e.badgeHidden, changed] = [hide, true];
    }
    for (let i = 0; i < order.length; i++) {
      const e = order[i];
      const box = e.label;
      let hide = false;
      if (box && e.rank >= 0) {
        hide = (e.rank === TYPE && e.badgeHidden) || this.rects.some((r) => overlap(box, r)) || crossed(box, e.id, this.lines);
        for (let j = 0; j < order.length && !hide; j++) hide = j !== i && shown(order[j]) && overlap(box, order[j].badge!);
        for (let j = 0; j < i && !hide; j++) hide = !order[j].hidden && order[j].label !== null && overlap(box, order[j].label!, SPACE);
      }
      if (e.hidden !== hide) [e.hidden, changed] = [hide, true];
    }
    if (changed) for (const f of this.listeners) f();
  }
}

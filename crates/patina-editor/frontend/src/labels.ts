// The labels above the wires, which hide when something covers them (spec 15.9).

import type { Rect } from './route';

interface Entry {
  id: string;
  label: Rect | null;
  badge: Rect | null;
  /** A lower rank shows first. A label with a rank below 0 always shows. */
  rank: number;
  hidden: boolean;
}

const overlap = (a: Rect, b: Rect) => a.l < b.r && b.l < a.r && a.t < b.b && b.t < a.b;

/**
 * The wire labels that hide. A label hides when a value badge, a node body or a label of a lower
 * rank covers it. The sockets and the pin tooltips still show the names.
 */
export class Labels {
  private entries = new Map<string, Entry>();
  private rects: readonly Rect[] = [];
  private order: Entry[] = [];
  private listeners = new Set<() => void>();
  private queued = false;

  /** The label box and the badge box of a wire, in flow coordinates. */
  set(id: string, label: Rect | null, badge: Rect | null, rank: number) {
    const e = this.entries.get(id);
    if (e) Object.assign(e, { label, badge, rank });
    else this.entries.set(id, { id, label, badge, rank, hidden: false });
    this.queue();
  }

  remove(id: string) {
    if (this.entries.delete(id)) this.queue();
  }

  setObstacles(rects: readonly Rect[]) {
    this.rects = rects;
    this.queue();
  }

  hidden = (id: string): boolean => this.entries.get(id)?.hidden ?? false;

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

  /** Shows the labels in the order of their rank. Each label hides when a shown box covers it. */
  compute() {
    const order = this.order;
    order.length = 0;
    let changed = false;
    for (const e of this.entries.values()) {
      if (e.label) order.push(e);
      else if (e.hidden) [e.hidden, changed] = [false, true];
    }
    order.sort((a, b) => a.rank - b.rank || (a.id < b.id ? -1 : a.id > b.id ? 1 : 0));
    for (let i = 0; i < order.length; i++) {
      const e = order[i];
      const box = e.label!;
      let hide = false;
      if (e.rank >= 0) {
        hide = this.rects.some((r) => overlap(box, r));
        for (const o of this.entries.values()) if (!hide && o !== e && o.badge) hide = overlap(box, o.badge);
        for (let j = 0; j < i && !hide; j++) hide = !order[j].hidden && overlap(box, order[j].label!);
      }
      if (e.hidden !== hide) [e.hidden, changed] = [hide, true];
    }
    if (changed) for (const f of this.listeners) f();
  }
}

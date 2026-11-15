// The SVG renderer: a React Flow custom edge. Each lane is a path. The frame loop changes only the
// stroke of the lanes that changed, and the text of the badges that changed.

import { createContext, memo, useContext, useLayoutEffect, useRef } from 'react';
import { useStore, type Edge, type EdgeProps, type ReactFlowState } from '@xyflow/react';
import type { Lanes } from './lanes';
import { ACCENT, BADGE_HEIGHT, BOOL_WIDTH, FAR_BUS_WIDTH, LANE_WIDTH, LIT, OFF, SELECT_EXTRA, WIRE } from './theme';
import { NEAR_ZOOM, type Wires } from './wires';

export type LaneEdgeData = { index: number; rev: number };
export type LaneEdgeType = Edge<LaneEdgeData, 'lane'>;

/** The lane and badge elements that are in the document now. */
export class SvgLive {
  private readonly laneEls: (Element | null)[];
  private readonly badgeEls: (Element | null)[];
  private readonly badgeVal: Float64Array;
  private readonly prev: Uint8Array;

  constructor(
    readonly wires: Wires,
    readonly lanes: Lanes,
  ) {
    this.laneEls = new Array(wires.graph.laneCount).fill(null);
    this.badgeEls = new Array(wires.graph.edges.length).fill(null);
    this.badgeVal = new Float64Array(wires.graph.edges.length);
    this.prev = new Uint8Array(lanes.bits.length);
  }

  registerLanes(group: Element) {
    for (const el of group.children) {
      const lane = Number(el.getAttribute('data-lane'));
      this.laneEls[lane] = el;
      el.setAttribute('stroke', this.lanes.bit(lane) ? LIT : OFF);
    }
  }

  unregisterLanes(group: Element) {
    for (const el of group.children) {
      const lane = Number(el.getAttribute('data-lane'));
      if (this.laneEls[lane] === el) this.laneEls[lane] = null;
    }
  }

  registerBadge(i: number, el: Element) {
    this.badgeEls[i] = el;
    this.badgeVal[i] = this.wires.value(i, this.lanes.bits);
    el.textContent = this.wires.badgeText(i, true, this.badgeVal[i]);
  }

  unregisterBadge(i: number, el: Element) {
    if (this.badgeEls[i] === el) this.badgeEls[i] = null;
  }

  /** Applies the newest lane states to the document. */
  apply() {
    const { bits } = this.lanes;
    const prev = this.prev;
    for (let b = 0; b < bits.length; b++) {
      const changed = bits[b] ^ prev[b];
      if (changed === 0) continue;
      for (let k = 0; k < 8; k++) {
        if (((changed >> k) & 1) === 0) continue;
        const el = this.laneEls[b * 8 + k];
        if (el) el.setAttribute('stroke', (bits[b] >> k) & 1 ? LIT : OFF);
      }
      prev[b] = bits[b];
    }
    for (let i = 0; i < this.badgeEls.length; i++) {
      const el = this.badgeEls[i];
      if (!el) continue;
      const v = this.wires.value(i, bits);
      if (v === this.badgeVal[i]) continue;
      this.badgeVal[i] = v;
      el.textContent = this.wires.badgeText(i, true, v);
    }
  }
}

export const SvgContext = createContext<SvgLive | null>(null);

const nearSelector = (s: ReactFlowState) => s.transform[2] >= NEAR_ZOOM;

function LaneEdgeView({ data, selected }: EdgeProps<LaneEdgeType>) {
  const live = useContext(SvgContext)!;
  const near = useStore(nearSelector);
  const lanesRef = useRef<SVGGElement>(null);
  const badgeRef = useRef<SVGTextElement>(null);
  const i = data!.index;
  const rev = data!.rev;
  const { wires } = live;
  const e = wires.graph.edges[i];
  const bus = e.width > 1;
  const showLanes = !bus || near;

  useLayoutEffect(() => {
    const g = lanesRef.current;
    if (!g) return;
    live.registerLanes(g);
    return () => live.unregisterLanes(g);
  }, [live, near, rev]);

  useLayoutEffect(() => {
    const el = badgeRef.current;
    if (!el || !near) return;
    live.registerBadge(i, el);
    return () => live.unregisterBadge(i, el);
  }, [live, near, i]);

  const center = wires.path(i, 0);
  const band = wires.band(i, near);
  const showBadge = near || bus;
  const bw = wires.badgeWidth(i, near);
  const lanes = [];
  if (showLanes) {
    for (let j = 0; j < e.width; j++) {
      lanes.push(<path key={j} data-lane={e.lane0 + j} d={bus ? wires.path(i, wires.laneOffset(i, j)) : center} />);
    }
  }
  return (
    <>
      {selected && <path d={center} fill="none" stroke={ACCENT} strokeWidth={band + SELECT_EXTRA} />}
      {bus && !near && <path d={center} fill="none" stroke={WIRE} strokeWidth={FAR_BUS_WIDTH} />}
      {showLanes && (
        <g ref={lanesRef} fill="none" strokeWidth={bus ? LANE_WIDTH : BOOL_WIDTH}>
          {lanes}
        </g>
      )}
      <path className="react-flow__edge-interaction" d={center} fill="none" stroke="transparent" strokeWidth={band + 12} />
      {showBadge && (
        <g className="lane-badge" transform={`translate(${wires.mid[i * 2]},${wires.badgeY(i, near)})`}>
          <rect x={-bw / 2} y={-BADGE_HEIGHT / 2} width={bw} height={BADGE_HEIGHT} rx={3} />
          <text ref={badgeRef} y={3.5}>
            {near ? '' : wires.badgeText(i, false, 0)}
          </text>
        </g>
      )}
    </>
  );
}

export const LaneEdge = memo(LaneEdgeView);

// The drawing of the circuit symbols of the nodes and the subgraph uses (spec 4.3), in one
// schematic style with lit leads. The layouts are in symbols.ts.

import type { CSSProperties } from 'react';
import { LANE_COLORS, type Socket } from './bus';
import { laneOffsets, lanePath, type Lead, type SymbolLayout } from './symbols';

const COLOR = {
  high: 'var(--accent)',
  /** A dark amber: the line is low. */
  low: '#7d6334',
  /** No live value. */
  none: 'var(--subtle)',
  bus: 'var(--wire)',
  line: 'var(--text)',
  body: 'var(--node)',
  /** The body of a gate whose output is high: the node color with 16% of the accent. */
  lit: '#443a2c',
  chip: '#1c1c1b',
  chipEdge: '#5d5d58',
  leg: '#a8a8a2',
};

// shortcut: a lane of a symbol has no hover tooltip, as the contacts of a box socket have; add one when a symbol needs it.
/** The lanes of a bus lead, one line for each lane, lit as the lanes of its wire (15.9). */
function LaneLines({ lead, socket }: { lead: Lead; socket: Socket | undefined }) {
  const style = { '--lane': `rgb(${LANE_COLORS[socket?.family ?? 0].join(' ')})` } as CSSProperties;
  return (
    <g className="symbol-lanes" style={style}>
      {laneOffsets(lead.lanes).map((d, i) => (
        <path key={i} d={lanePath(lead, d)} className={socket?.bits[i] === '1' ? 'on' : undefined} />
      ))}
    </g>
  );
}

/** The metal legs of a chip pin: one leg for each lane of a bus. */
function Legs({ lead, high, socket }: { lead: Lead; high: boolean; socket: Socket | undefined }) {
  const x = lead.key.startsWith('i:') ? lead.body[0] - 6 : lead.body[0];
  if (lead.lanes < 2) return <rect x={x} y={lead.body[1] - 2.5} width={6} height={5} fill={high ? COLOR.high : COLOR.leg} />;
  return laneOffsets(lead.lanes).map((d, i) => <rect key={i} x={x} y={lead.body[1] + d - 1.5} width={6} height={3} fill={socket?.bits[i] === '1' ? COLOR.high : COLOR.leg} />);
}

/** The drawing of a symbol. A lead is amber when its level is high, dark amber when low, and gray with no live value. */
export function SymbolBody({ layout, levels, sockets, far }: { layout: SymbolLayout; levels?: Readonly<Record<string, boolean>>; sockets?: Readonly<Record<string, Socket>>; far: boolean }) {
  const color = (l: Lead) => {
    const level = levels?.[l.key];
    if (level === undefined) return l.bus ? COLOR.bus : COLOR.none;
    return level ? COLOR.high : COLOR.low;
  };
  const lit = layout.lit !== undefined && levels?.[layout.lit] === true;
  const fills = { body: lit ? COLOR.lit : COLOR.body, chip: COLOR.chip, mark: COLOR.chipEdge, none: 'none' };
  return (
    <svg className="symbol" width={layout.width} height={layout.height} viewBox={`0 0 ${layout.width} ${layout.height}`} aria-hidden="true">
      {layout.leads.map((l) =>
        l.lanes > 1 ? (
          <LaneLines key={l.key} lead={l} socket={sockets?.[l.key]} />
        ) : (
          <path key={l.key} d={lanePath(l, 0)} fill="none" stroke={color(l)} strokeWidth={l.bus ? 3 : 1.5} strokeLinejoin="round" />
        ),
      )}
      {layout.parts.map((p, i) => (
        <path key={i} d={p.d} fill={fills[p.fill]} stroke={p.width ? COLOR.bus : p.fill === 'chip' || p.fill === 'mark' ? COLOR.chipEdge : COLOR.line} strokeWidth={p.width ?? 1.5} strokeLinejoin="round" strokeLinecap={p.width ? 'round' : undefined} />
      ))}
      {layout.leads.map((l) => l.leg && <Legs key={`leg ${l.key}`} lead={l} high={levels?.[l.key] === true} socket={sockets?.[l.key]} />)}
      {!far &&
        layout.texts.map((t, i) => (
          <text key={`t${i}`} x={t.at[0]} y={t.at[1]} textAnchor={t.anchor} className={t.kind === 'label' ? 'symbol-label' : 'symbol-pin'}>
            {t.text}
          </text>
        ))}
    </svg>
  );
}

// A bus socket on a node (spec 15.9): the pin name and lane range, one contact for each lane with
// its name and LED, and the leads that fan in from the contacts to the lanes of the ribbon.

import { useContext, useSyncExternalStore, type CSSProperties } from 'react';
import { HoverContext, LANE_COLORS, PITCH, hoveredLane, laneTip, type Socket } from './bus';

/** The label line, the pitch of the contacts and the space under the last contact. */
const HEAD = 18;
const CONTACT = 12;
const FOOT = 4;
/** The horizontal length of a lead, from the node edge to the LED. */
const LEAD = 14;

export function socketHeight(n: number): number {
  return HEAD + n * CONTACT + FOOT;
}

/** The center of the pin of a socket at `top`, where the ribbon meets the node edge. */
export function socketPin(top: number, n: number): number {
  return top + HEAD + (n * CONTACT) / 2;
}

interface Props {
  socket: Socket;
  output: boolean;
  /** The top of the row of the socket in the node. */
  top: number;
  /** False on a public pin, whose header already shows the name. */
  named?: boolean;
}

export function BusSocket({ socket, output, top, named = true }: Props) {
  const hover = useContext(HoverContext);
  const hot = useSyncExternalStore(hover.subscribe, () => hoveredLane(hover.get(), socket.from, socket.to));
  const { bus, names, bits } = socket;
  const n = bits.length;
  const mid = HEAD + (n * CONTACT) / 2;
  const style = { top, height: socketHeight(n), '--lane': `rgb(${LANE_COLORS[socket.family].join(' ')})` } as CSSProperties;
  const classes = ['bus-socket', output ? 'out' : 'in'];
  if (socket.stale) classes.push('stale');
  return (
    <div className={classes.join(' ')} style={style}>
      <div className="bus-socket-head" title={`${bus}: ${n} lanes`}>
        {named && bus}
        <span className="bus-range">
          [{n - 1}:0]
        </span>
      </div>
      {/* The leads go from the lanes at the node edge (x = 0) to the contacts. An output mirrors them. */}
      <svg className="bus-leads" width={LEAD} height={socketHeight(n)} aria-hidden="true">
        {names.map((_, i) => {
          const edge = mid + (i - (n - 1) / 2) * PITCH;
          const contact = HEAD + i * CONTACT + CONTACT / 2;
          const on = bits[i] === '1' ? ' on' : '';
          return <path key={i} className={hot === i ? 'hot' : on} d={`M0 ${edge}C${LEAD / 2} ${edge} ${LEAD / 2} ${contact} ${LEAD} ${contact}`} />;
        })}
      </svg>
      {names.map((name, i) => (
        <div
          key={i}
          className={`bus-contact${bits[i] === '1' ? ' on' : ''}${hot === i ? ' hot' : ''}`}
          style={{ top: HEAD + i * CONTACT }}
          title={laneTip(bus, names, bits, i, socket.live)}
          onPointerEnter={() => hover.set({ from: socket.from, to: socket.to, lane: i })}
          onPointerLeave={() => hover.set(null)}
        >
          <span className="bus-led" />
          <span className="bus-name">{name}</span>
        </div>
      ))}
    </div>
  );
}

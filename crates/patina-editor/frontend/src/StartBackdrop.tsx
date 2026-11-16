// The picture behind the start screen (15.10): a node network of a small computer. It is only
// for the look, so it has no meaning for the program and screen readers skip it.

interface Box {
  x: number;
  y: number;
  title: string;
  ins: number;
  outs: number;
}

const W = 150;
const HEAD = 26;
const ROW = 18;

const NODES: Box[] = [
  { x: 60, y: 90, title: 'clock', ins: 0, outs: 1 },
  { x: 70, y: 300, title: 'switch', ins: 0, outs: 1 },
  { x: 60, y: 470, title: 'switch', ins: 0, outs: 1 },
  { x: 300, y: 160, title: 'counter-4', ins: 2, outs: 4 },
  { x: 320, y: 420, title: 'nand', ins: 2, outs: 1 },
  { x: 120, y: 690, title: 'rom-8x7', ins: 3, outs: 4 },
  { x: 560, y: 70, title: 'decoder-4', ins: 4, outs: 3 },
  { x: 580, y: 760, title: 'full-adder', ins: 3, outs: 2 },
  { x: 380, y: 880, title: 'register', ins: 2, outs: 2 },
  { x: 1050, y: 60, title: 'mux-4', ins: 4, outs: 2 },
  { x: 1100, y: 330, title: 'cpu', ins: 3, outs: 4 },
  { x: 1000, y: 700, title: 'adder-4', ins: 4, outs: 3 },
  { x: 1340, y: 140, title: 'hex-7seg', ins: 4, outs: 3 },
  { x: 1380, y: 560, title: 'display', ins: 3, outs: 0 },
  { x: 1300, y: 860, title: 'readout', ins: 2, outs: 0 },
];

/** [from node, output, to node, input, on]. An "on" wire carries a 1 and glows. */
const WIRES: [number, number, number, number, boolean][] = [
  [0, 0, 3, 0, true],
  [1, 0, 3, 1, false],
  [1, 0, 4, 0, false],
  [2, 0, 4, 1, true],
  [3, 0, 6, 0, true],
  [3, 1, 6, 1, false],
  [3, 2, 5, 0, true],
  [4, 0, 7, 0, true],
  [5, 0, 7, 1, false],
  [5, 1, 8, 0, true],
  [6, 0, 9, 0, true],
  [6, 2, 10, 0, false],
  [7, 0, 11, 0, true],
  [8, 1, 11, 2, false],
  [9, 0, 12, 0, true],
  [9, 1, 10, 1, true],
  [10, 0, 12, 2, false],
  [10, 2, 13, 0, true],
  [11, 1, 13, 2, true],
  [11, 2, 14, 0, false],
  [12, 1, 13, 1, true],
];

const height = (n: Box) => HEAD + Math.max(n.ins, n.outs, 1) * ROW + 10;
const pin = (n: Box, i: number, out: boolean) => [out ? n.x + W : n.x, n.y + HEAD + 9 + i * ROW] as const;

function wirePath([a, o, b, i]: (typeof WIRES)[number]): string {
  const [x1, y1] = pin(NODES[a], o, true);
  const [x2, y2] = pin(NODES[b], i, false);
  const d = Math.max(60, Math.abs(x2 - x1) / 2);
  return `M${x1} ${y1}C${x1 + d} ${y1} ${x2 - d} ${y2} ${x2} ${y2}`;
}

export function StartBackdrop() {
  return (
    <svg className="start-backdrop" viewBox="0 0 1600 1000" preserveAspectRatio="xMidYMid slice" aria-hidden="true" focusable="false">
      {WIRES.map((w, k) => (
        <g key={k} className={w[4] ? 'bd-wire on' : 'bd-wire'}>
          {w[4] && <path className="bd-glow" d={wirePath(w)} />}
          <path d={wirePath(w)} />
          {w[4] && <path className="bd-pulse" d={wirePath(w)} style={{ animationDelay: `${-k * 0.37}s` }} />}
        </g>
      ))}
      {NODES.map((n, k) => (
        <g key={k} className="bd-node">
          <rect x={n.x} y={n.y} width={W} height={height(n)} rx="6" />
          <path className="bd-head" d={`M${n.x} ${n.y + HEAD}V${n.y + 6}a6 6 0 0 1 6 -6H${n.x + W - 6}a6 6 0 0 1 6 6V${n.y + HEAD}Z`} />
          <text x={n.x + 10} y={n.y + 17}>
            {n.title}
          </text>
          {Array.from({ length: n.ins }, (_, i) => (
            <circle key={`i${i}`} cx={pin(n, i, false)[0]} cy={pin(n, i, false)[1]} r="3.5" />
          ))}
          {Array.from({ length: n.outs }, (_, i) => (
            <circle key={`o${i}`} cx={pin(n, i, true)[0]} cy={pin(n, i, true)[1]} r="3.5" />
          ))}
        </g>
      ))}
    </svg>
  );
}

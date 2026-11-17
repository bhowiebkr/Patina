// The picture behind the start screen (15.10): a node network of a small computer. It is only
// for the look, so it has no meaning for the program and screen readers skip it.

import { useEffect, useMemo, useState } from 'react';
import { type BdNode, type BdWire, HEAD, NODE_W, ROW, network, nodeHeight } from './backdrop';

const pin = (n: BdNode, i: number, out: boolean) => [out ? n.x + NODE_W : n.x, n.y + HEAD + 9 + i * ROW] as const;

function wirePath(nodes: BdNode[], [a, o, b, i]: BdWire): string {
  const [x1, y1] = pin(nodes[a], o, true);
  const [x2, y2] = pin(nodes[b], i, false);
  const d = Math.max(60, Math.abs(x2 - x1) / 2);
  return `M${x1} ${y1}C${x1 + d} ${y1} ${x2 - d} ${y2} ${x2} ${y2}`;
}

function useWindowSize(): [number, number] {
  const [size, setSize] = useState<[number, number]>(() => [window.innerWidth, window.innerHeight]);
  useEffect(() => {
    const resize = () => setSize([window.innerWidth, window.innerHeight]);
    window.addEventListener('resize', resize);
    return () => window.removeEventListener('resize', resize);
  }, []);
  return size;
}

export function StartBackdrop() {
  const [width, height] = useWindowSize();
  const { nodes, wires } = useMemo(() => network(width, height), [width, height]);
  return (
    <svg className="start-backdrop" aria-hidden="true" focusable="false">
      {wires.map((w, k) => {
        const d = wirePath(nodes, w);
        return (
          <g key={k} className={w[4] ? 'bd-wire on' : 'bd-wire'}>
            {w[4] && <path className="bd-glow" d={d} />}
            <path d={d} />
            {w[4] && k % 2 === 0 && <path className="bd-pulse" d={d} style={{ animationDelay: `${-k * 0.37}s` }} />}
          </g>
        );
      })}
      {nodes.map((n, k) => (
        <g key={k} className="bd-node">
          <rect x={n.x} y={n.y} width={NODE_W} height={nodeHeight(n)} rx="6" />
          <path className="bd-head" d={`M${n.x} ${n.y + HEAD}V${n.y + 6}a6 6 0 0 1 6 -6H${n.x + NODE_W - 6}a6 6 0 0 1 6 6V${n.y + HEAD}Z`} />
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

// The node network behind the start screen (15.10). It has a fixed scale, so a larger screen
// shows more nodes, not larger nodes.

export interface BdNode {
  x: number;
  y: number;
  title: string;
  ins: number;
  outs: number;
}

/** [from node, output, to node, input, on]. An "on" wire carries a 1 and glows. */
export type BdWire = [number, number, number, number, boolean];

export const NODE_W = 150;
export const HEAD = 26;
export const ROW = 18;
const CELL_W = 270;
const CELL_H = 200;
const TITLES = ['clock', 'switch', 'nand', 'counter-4', 'decoder-4', 'full-adder', 'register', 'mux-4', 'cpu', 'adder-4', 'hex-7seg', 'display', 'readout', 'rom-16x7', 'ram-16', 'lcd', 'alu-8', 'bus'];

export const nodeHeight = (n: BdNode) => HEAD + Math.max(n.ins, n.outs, 1) * ROW + 10;

/** A number in [0, 1) from the cell only, so a cell keeps its node when the screen size changes. */
function hash(col: number, row: number, salt: number): number {
  let h = Math.imul(col, 374761393) ^ Math.imul(row, 668265263) ^ Math.imul(salt, 1274126177);
  h = Math.imul(h ^ (h >>> 13), 1103515245);
  return ((h ^ (h >>> 16)) >>> 0) / 4294967296;
}

/** The nodes and wires that cover a screen of this size. Each wire goes to the next column. */
export function network(width: number, height: number): { nodes: BdNode[]; wires: BdWire[] } {
  const cols = Math.ceil(width / CELL_W);
  const rows = Math.ceil(height / CELL_H);
  const nodes: BdNode[] = [];
  const at = new Map<string, number>();
  for (let c = -1; c <= cols; c++) {
    for (let r = -1; r <= rows; r++) {
      if (hash(c, r, 1) < 0.2) continue;
      at.set(`${c},${r}`, nodes.length);
      const pick = (salt: number, n: number) => Math.floor(hash(c, r, salt) * n);
      nodes.push({ x: c * CELL_W + pick(2, 80), y: r * CELL_H + pick(3, 60), title: TITLES[pick(4, TITLES.length)], ins: 1 + pick(5, 4), outs: 1 + pick(6, 3) });
    }
  }
  const wires: BdWire[] = [];
  for (const [key, a] of at) {
    const [c, r] = key.split(',').map(Number);
    for (let o = 0; o < nodes[a].outs; o++) {
      const b = at.get(`${c + 1},${r + Math.floor(hash(c, r, 10 + o) * 3) - 1}`);
      if (b !== undefined) wires.push([a, o, b, Math.floor(hash(c, r, 20 + o) * nodes[b].ins), hash(c, r, 30 + o) < 0.45]);
    }
  }
  return { nodes, wires };
}

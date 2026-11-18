// The circuit symbols of the nodes and the subgraph uses (spec 4.3): the logic gates, the blocks,
// a DIP chip and a bus bar, in one schematic style with lit leads.

import type { Pin, Shape, Symbol } from '../api';

/** The length of a pin lead outside the body. */
const LEAD = 14;
/** The distance between two pins on one edge. */
const PITCH = 20;
/** The width of one character of a pin name (10px) and of a label (11px bold). */
const CHAR = 6;
const LABEL_CHAR = 7;

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

type Point = [number, number];

/** A pin lead from the pin place `end` to the edge of the body. A chip pin has a metal leg. */
interface Lead {
  key: string;
  end: Point;
  /** The corner of a lead that turns, for example from the left edge up to the bottom of a block. */
  bend?: Point;
  body: Point;
  bus: boolean;
  leg: boolean;
}

interface Part {
  d: string;
  fill: 'body' | 'chip' | 'mark' | 'none';
  /** The stroke width of a bus bar. */
  width?: number;
}

interface Text {
  at: Point;
  text: string;
  anchor: 'start' | 'middle' | 'end';
  kind: 'pin' | 'label';
}

export interface SymbolLayout {
  width: number;
  height: number;
  leads: Lead[];
  parts: Part[];
  texts: Text[];
  /** The output whose high level tints the body of a gate. */
  lit?: string;
}

const nameOf = (p: Pin) => p.name ?? p.key;
const isBus = (p: Pin) => p.ty !== 'bool' && p.ty !== '';
const CONTROL = /^(sel|s|op|[sf]\d)$/i;
const isControl = (p: Pin) => CONTROL.test(p.key) || CONTROL.test(p.name ?? '');
const CLOCK = /^(clk|clock)$/i;
const isClock = (p: Pin) => CLOCK.test(p.key) || CLOCK.test(p.name ?? '');
/** `count` places at PITCH, with `mid` in the middle. */
const spread = (count: number, mid: number) => Array.from({ length: count }, (_, i) => mid + (i - (count - 1) / 2) * PITCH);
const widest = (names: readonly string[]) => Math.max(0, ...names.map((n) => n.length)) * CHAR;
const lead = (side: 'i' | 'o', p: Pin, end: Point, body: Point, leg = false): Lead => ({ key: `${side}:${p.key}`, end, body, bus: isBus(p), leg });
const pinText = (at: Point, text: string, anchor: Text['anchor']): Text => ({ at: [at[0], at[1] + 3.5], text, anchor, kind: 'pin' });
/** The middle of the room between the input names and the output names. */
const middle = (x0: number, long: number, inW: number, outW: number) => x0 + 5 + inW + (long - 10 - inW - outW) / 2;

/** The layout of a symbol. The pin places are the `end` of the leads, by "i:<key>" and "o:<key>". */
export function symbolLayout(symbol: Symbol, name: string, inputs: readonly Pin[], outputs: readonly Pin[]): SymbolLayout {
  const label = symbol.label || name.toUpperCase();
  switch (symbol.shape) {
    case 'mux':
    case 'alu':
      return block(symbol.shape, label, inputs, outputs);
    case 'adder':
      return adder(label, inputs, outputs);
    case 'chip':
      return chip(label, inputs, outputs);
    case 'bus':
      return bus(label, inputs, outputs);
    default:
      return gate(symbol.shape, inputs, outputs);
  }
}

/** A logic gate with the distinctive shape of IEEE 91. */
function gate(shape: Shape, inputs: readonly Pin[], outputs: readonly Pin[]): SymbolLayout {
  const one = shape === 'not' || shape === 'buffer';
  const bubble = shape === 'nand' || shape === 'nor' || shape === 'xnor' || shape === 'not';
  const or = shape === 'or' || shape === 'nor' || shape === 'xor' || shape === 'xnor';
  const xor = shape === 'xor' || shape === 'xnor';
  const height = Math.max(40, inputs.length * PITCH, outputs.length * PITCH);
  const mid = height / 2;
  const tall = one ? 28 : Math.min(height, 60);
  const [y0, y1] = [mid - tall / 2, mid + tall / 2];
  const long = one ? 26 : Math.round(tall * 1.2);
  const x0 = LEAD;
  const back = x0 + (xor ? 6 : 0);
  const tip = back + long;
  const out = tip + (bubble ? 8 : 0);
  const curve = 0.22 * long;
  const parts: Part[] = [];
  if (one) parts.push({ d: `M${back} ${y0}L${tip} ${mid}L${back} ${y1}Z`, fill: 'body' });
  else if (or) parts.push({ d: `M${back} ${y0}C${back + 0.55 * long} ${y0} ${back + 0.85 * long} ${y0 + 0.18 * tall} ${tip} ${mid}C${back + 0.85 * long} ${y1 - 0.18 * tall} ${back + 0.55 * long} ${y1} ${back} ${y1}Q${back + curve} ${mid} ${back} ${y0}Z`, fill: 'body' });
  else parts.push({ d: `M${back} ${y0}H${tip - tall / 2}A${tall / 2} ${tall / 2} 0 0 1 ${tip - tall / 2} ${y1}H${back}Z`, fill: 'body' });
  if (xor) parts.push({ d: `M${x0} ${y0}Q${x0 + curve} ${mid} ${x0} ${y1}`, fill: 'none' });
  if (bubble) parts.push({ d: `M${tip} ${mid}a4 4 0 1 0 8 0a4 4 0 1 0 -8 0`, fill: 'body' });
  const ins = spread(inputs.length, mid);
  // An input past the body ends on a line that extends the back, as on a wide gate of a schematic.
  if (ins.length > 0 && (ins[0] < y0 || ins[ins.length - 1] > y1)) parts.push({ d: `M${x0} ${Math.min(ins[0], y0)}V${Math.max(ins[ins.length - 1], y1)}`, fill: 'none' });
  // The point on the curved back of an OR shape: a quadratic curve whose y is linear in t.
  const backAt = (y: number) => {
    const t = Math.min(1, Math.max(0, (y - y0) / tall));
    return or ? x0 + 2 * t * (1 - t) * curve : back;
  };
  const leads = [...inputs.map((p, i) => lead('i', p, [0, ins[i]], [backAt(ins[i]), ins[i]])), ...spread(outputs.length, mid).map((y, i) => lead('o', outputs[i], [out + LEAD, y], [out, y]))];
  return { width: out + LEAD, height, leads, parts, texts: [], lit: outputs[0] && `o:${outputs[0].key}` };
}

/**
 * The control inputs on the bottom edge, from (x0, tall) to (x0 + long, tall - slant). Each lead
 * comes from a pin place on the left edge, so its wire meets it as a left pin (15.9). A lead further
 * right is lower, so the leads do not cross.
 */
function controlLeads(controls: readonly Pin[], x0: number, long: number, tall: number, slant: number, leads: Lead[], texts: Text[]) {
  controls.forEach((p, i) => {
    const x = Math.round(x0 + (long * (i + 1)) / (controls.length + 1));
    const edge = tall - (slant * (x - x0)) / long;
    const y = tall + 6 + i * 8;
    leads.push({ ...lead('i', p, [0, y], [x, edge]), bend: [x, y] });
    texts.push({ at: [x, edge - 5], text: nameOf(p), anchor: 'middle', kind: 'pin' });
  });
}

/** A multiplexer (a trapezoid) or an ALU (a chevron with a notch between its two operands). */
function block(shape: 'mux' | 'alu', label: string, inputs: readonly Pin[], outputs: readonly Pin[]): SymbolLayout {
  const controls = inputs.filter(isControl);
  const data = inputs.filter((p) => !isControl(p));
  const alu = shape === 'alu';
  const upper = Math.ceil(data.length / 2);
  const rows = alu ? Math.max(data.length + 1, outputs.length + 2, 3) : Math.max(data.length, outputs.length + 1, 2);
  const tall = rows * PITCH + 16;
  const mid = tall / 2;
  const slant = alu ? tall * 0.22 : Math.min(16, tall / 4);
  const [inW, outW] = [widest(data.map(nameOf)), widest(outputs.map(nameOf))];
  const long = Math.max(alu ? 64 : 52, inW + outW + label.length * LABEL_CHAR + 24, controls.length * 24 + 16);
  const [x0, x1] = [LEAD, LEAD + long];
  // The ALU operands go above and below the notch.
  const places = spread(data.length + (alu ? 1 : 0), mid);
  const ins = alu ? places.filter((_, i) => i !== upper) : places;
  const notch = alu ? `V${places[upper] + 7}L${x0 + 10} ${places[upper]}L${x0} ${places[upper] - 7}` : '';
  const parts: Part[] = [{ d: `M${x0} 0L${x1} ${slant}V${tall - slant}L${x0} ${tall}${notch}Z`, fill: 'body' }];
  const outs = spread(outputs.length, mid);
  const leads = [...data.map((p, i) => lead('i', p, [0, ins[i]], [x0, ins[i]])), ...outputs.map((p, i) => lead('o', p, [x1 + LEAD, outs[i]], [x1, outs[i]]))];
  const texts = [
    ...data.map((p, i) => pinText([x0 + 5, ins[i]], nameOf(p), 'start')),
    ...outputs.map((p, i) => pinText([x1 - 5, outs[i]], nameOf(p), 'end')),
    { at: [middle(x0, long, inW, outW), mid + 4] as Point, text: label, anchor: 'middle' as const, kind: 'label' as const },
  ];
  controlLeads(controls, x0, long, tall, slant, leads, texts);
  return { width: x1 + LEAD, height: tall + (controls.length > 0 ? controls.length * 8 + 2 : 0), leads, parts, texts };
}

/** An adder: a block with a sigma sign and its label under it. */
function adder(label: string, inputs: readonly Pin[], outputs: readonly Pin[]): SymbolLayout {
  const rows = Math.max(inputs.length, outputs.length, 2);
  const tall = rows * PITCH + 20;
  const mid = tall / 2;
  const [inW, outW] = [widest(inputs.map(nameOf)), widest(outputs.map(nameOf))];
  const long = inW + outW + Math.max(24, label.length * CHAR) + 26;
  const [x0, x1] = [LEAD, LEAD + long];
  const [cx, cy] = [middle(x0, long, inW, outW), mid - 6];
  const parts: Part[] = [
    { d: `M${x0 + 4} 0H${x1 - 4}Q${x1} 0 ${x1} 4V${tall - 4}Q${x1} ${tall} ${x1 - 4} ${tall}H${x0 + 4}Q${x0} ${tall} ${x0} ${tall - 4}V4Q${x0} 0 ${x0 + 4} 0Z`, fill: 'body' },
    { d: `M${cx + 7} ${cy - 9}H${cx - 7}L${cx + 1} ${cy}L${cx - 7} ${cy + 9}H${cx + 7}`, fill: 'none' },
  ];
  const [ins, outs] = [spread(inputs.length, mid), spread(outputs.length, mid)];
  const leads = [...inputs.map((p, i) => lead('i', p, [0, ins[i]], [x0, ins[i]])), ...outputs.map((p, i) => lead('o', p, [x1 + LEAD, outs[i]], [x1, outs[i]]))];
  const texts = [...inputs.map((p, i) => pinText([x0 + 5, ins[i]], nameOf(p), 'start')), ...outputs.map((p, i) => pinText([x1 - 5, outs[i]], nameOf(p), 'end')), pinText([cx, cy + 20], label, 'middle')];
  return { width: x1 + LEAD, height: tall, leads, parts, texts };
}

/** A DIP chip: a black body with a notch, a pin 1 mark, metal legs, the pin names and the label. */
function chip(label: string, inputs: readonly Pin[], outputs: readonly Pin[]): SymbolLayout {
  const rows = Math.max(inputs.length, outputs.length, 1);
  const top = 14;
  const tall = top + rows * PITCH + 6;
  const inW = Math.max(0, ...inputs.map((p) => nameOf(p).length * CHAR + (isClock(p) ? 9 : 0)));
  const outW = widest(outputs.map(nameOf));
  const long = Math.max(72, inW + outW + label.length * LABEL_CHAR + 36);
  const [x0, x1] = [LEAD, LEAD + long];
  const cx = x0 + long / 2;
  // A side with fewer pins is in the middle of the rows.
  const at = (count: number, i: number) => top + PITCH / 2 + (Math.floor((rows - count) / 2) + i) * PITCH;
  const parts: Part[] = [
    { d: `M${x0} 3Q${x0} 0 ${x0 + 3} 0H${cx - 6}A6 6 0 0 0 ${cx + 6} 0H${x1 - 3}Q${x1} 0 ${x1} 3V${tall - 3}Q${x1} ${tall} ${x1 - 3} ${tall}H${x0 + 3}Q${x0} ${tall} ${x0} ${tall - 3}Z`, fill: 'chip' },
    { d: `M${x0 + 5} 7a2 2 0 1 0 4 0a2 2 0 1 0 -4 0`, fill: 'mark' },
  ];
  const leads: Lead[] = [];
  const texts: Text[] = [];
  inputs.forEach((p, i) => {
    const y = at(inputs.length, i);
    leads.push(lead('i', p, [0, y], [x0, y], true));
    const clock = isClock(p);
    if (clock) parts.push({ d: `M${x0} ${y - 5}L${x0 + 7} ${y}L${x0} ${y + 5}`, fill: 'none' });
    texts.push(pinText([x0 + 5 + (clock ? 9 : 0), y], nameOf(p), 'start'));
  });
  outputs.forEach((p, i) => {
    const y = at(outputs.length, i);
    leads.push(lead('o', p, [x1 + LEAD, y], [x1, y], true));
    texts.push(pinText([x1 - 5, y], nameOf(p), 'end'));
  });
  texts.push({ at: [middle(x0, long, inW, outW), top + (rows * PITCH) / 2 + 4], text: label, anchor: 'middle', kind: 'label' });
  return { width: x1 + LEAD, height: tall, leads, parts, texts };
}

/** The digits of pin names that share a prefix, for example 0 to 7 of b0 to b7, or else the names. */
function tags(pins: readonly Pin[]): string[] {
  const names = pins.map(nameOf);
  const found = names.map((n) => /^(.*?)(\d+)$/.exec(n));
  return found.every((m) => m && m[1] === found[0]![1]) ? found.map((m) => m![2]) : names;
}

/** A bus bar: the lines of the side with more pins tap the bar, and the other side joins its middle. */
function bus(label: string, inputs: readonly Pin[], outputs: readonly Pin[]): SymbolLayout {
  const split = outputs.length >= inputs.length;
  const [many, few] = split ? [outputs, inputs] : [inputs, outputs];
  const rows = Math.max(many.length, 1);
  const top = 18;
  const height = top + rows * PITCH;
  const mid = top + (rows * PITCH) / 2;
  const names = tags(many);
  const stub = Math.max(26, widest(names) + 14);
  const bar = split ? LEAD + 10 : stub;
  const width = bar + (split ? stub : LEAD + 10);
  const [manyX, fewX] = split ? [width, 0] : [0, width];
  const ys = spread(many.length, mid);
  const parts: Part[] = [{ d: `M${bar} ${top + 2}V${height - 2}`, fill: 'none', width: 6 }];
  const leads = [...many.map((p, i) => lead(split ? 'o' : 'i', p, [manyX, ys[i]], [bar, ys[i]])), ...spread(few.length, mid).map((y, i) => lead(split ? 'i' : 'o', few[i], [fewX, y], [bar, y]))];
  const texts = [...many.map((_, i) => ({ at: [split ? bar + 6 : bar - 6, ys[i] - 3] as Point, text: names[i], anchor: split ? ('start' as const) : ('end' as const), kind: 'pin' as const })), { at: [bar, top - 6] as Point, text: label, anchor: 'middle' as const, kind: 'label' as const }];
  return { width, height, leads, parts, texts };
}

/** The drawing of a symbol. A lead is amber when its level is high, dark amber when low, and gray with no live value. */
export function SymbolBody({ layout, levels, far }: { layout: SymbolLayout; levels?: Readonly<Record<string, boolean>>; far: boolean }) {
  const color = (l: Lead) => {
    const level = levels?.[l.key];
    if (level === undefined) return l.bus ? COLOR.bus : COLOR.none;
    return level ? COLOR.high : COLOR.low;
  };
  const lit = layout.lit !== undefined && levels?.[layout.lit] === true;
  const fills = { body: lit ? COLOR.lit : COLOR.body, chip: COLOR.chip, mark: COLOR.chipEdge, none: 'none' };
  return (
    <svg className="symbol" width={layout.width} height={layout.height} viewBox={`0 0 ${layout.width} ${layout.height}`} aria-hidden="true">
      {layout.leads.map((l) => (
        <path key={l.key} d={`M${l.end[0]} ${l.end[1]}${l.bend ? `L${l.bend[0]} ${l.bend[1]}` : ''}L${l.body[0]} ${l.body[1]}`} fill="none" stroke={color(l)} strokeWidth={l.bus ? 3 : 1.5} strokeLinejoin="round" />
      ))}
      {layout.parts.map((p, i) => (
        <path key={i} d={p.d} fill={fills[p.fill]} stroke={p.width ? COLOR.bus : p.fill === 'chip' || p.fill === 'mark' ? COLOR.chipEdge : COLOR.line} strokeWidth={p.width ?? 1.5} strokeLinejoin="round" strokeLinecap={p.width ? 'round' : undefined} />
      ))}
      {layout.leads.map((l) => l.leg && <rect key={`leg ${l.key}`} x={l.key.startsWith('i:') ? l.body[0] - 6 : l.body[0]} y={l.body[1] - 2.5} width={6} height={5} fill={levels?.[l.key] ? COLOR.high : COLOR.leg} />)}
      {!far &&
        layout.texts.map((t, i) => (
          <text key={`t${i}`} x={t.at[0]} y={t.at[1]} textAnchor={t.anchor} className={t.kind === 'label' ? 'symbol-label' : 'symbol-pin'}>
            {t.text}
          </text>
        ))}
    </svg>
  );
}

// The layouts of the circuit symbols (spec 4.3): the logic gates, the blocks, a DIP chip, a bus
// bar and a custom outline. A bus pin is a bar with one line for each lane (15.9), and its room
// keeps the bars apart.

import type { Pin, Shape, Symbol } from '../api.ts';
import { PITCH as LANE, socketBar } from './bus.ts';

/** The length of a pin lead outside the body. */
const LEAD = 14;
/** The distance between two one-line pins on one edge. */
const PITCH = 20;
/** The width of one character of a pin name (10px) and of a label (11px bold). */
const CHAR = 6;
const LABEL_CHAR = 7;

export type Point = [number, number];

/** A pin lead from the pin place `end` to the edge of the body. A chip pin has a metal leg. */
export interface Lead {
  key: string;
  end: Point;
  /** The corner of a lead that turns, for example from the left edge up to the bottom of a block. */
  bend?: Point;
  body: Point;
  bus: boolean;
  leg: boolean;
  /** The lanes of a bus pin, each drawn as its own line, else 0 or 1 for one line. */
  lanes: number;
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
  /** The fill of the body as "#rrggbb", in place of the theme color. */
  color?: string;
  /** The place of the point (0, 0) of a custom outline in the layout. */
  origin?: Point;
}

/** The lane count of the pin of a lead key "i:<key>" or "o:<key>", 0 for a pin with no lanes. */
export type Lanes = (key: string) => number;
type Count = (side: 'i' | 'o', p: Pin) => number;

const nameOf = (p: Pin) => p.name ?? p.key;
const isBus = (p: Pin) => p.ty !== 'bool' && p.ty !== '';
const CONTROL = /^(sel|s|op|[sf]\d)$/i;
const isControl = (p: Pin) => CONTROL.test(p.key) || CONTROL.test(p.name ?? '');
const CLOCK = /^(clk|clock)$/i;
const isClock = (p: Pin) => CLOCK.test(p.key) || CLOCK.test(p.name ?? '');
const widest = (names: readonly string[]) => Math.max(0, ...names.map((n) => n.length)) * CHAR;
const lead = (side: 'i' | 'o', p: Pin, end: Point, body: Point, lanes: number, leg = false): Lead => ({ key: `${side}:${p.key}`, end, body, bus: isBus(p), leg, lanes });
const pinText = (at: Point, text: string, anchor: Text['anchor']): Text => ({ at: [at[0], at[1] + 3.5], text, anchor, kind: 'pin' });
/** The middle of the room between the input names and the output names. */
const middle = (x0: number, long: number, inW: number, outW: number) => x0 + 5 + inW + (long - 10 - inW - outW) / 2;
const sum = (rooms: readonly number[]) => rooms.reduce((s, r) => s + r, 0);

/** The half width of the lanes of a lead, 0 for one line. */
const halfOf = (n: number) => (n > 1 ? (n * LANE) / 2 : 0);

/** The room of a pin on its edge: one PITCH, or its bar and a gap, so that two bars never touch. */
export const roomOf = (n: number) => (n > 1 ? Math.max(PITCH, Math.ceil((socketBar(n) + 6) / 10) * 10) : PITCH);

/** The centers of pins with these rooms, side by side, with `mid` in the middle. */
function stack(rooms: readonly number[], mid: number): number[] {
  let y = mid - sum(rooms) / 2;
  return rooms.map((r) => {
    y += r;
    return y - r / 2;
  });
}

/** The offsets of the lanes of a lead from its center line, the most significant lane first. */
export const laneOffsets = (n: number) => Array.from({ length: n }, (_, i) => (i - (n - 1) / 2) * LANE);

/** The line of the lane at the offset `d` of a lead. A lane that turns keeps its offset on both legs, so that no lanes cross. */
export function lanePath(l: Lead, d: number): string {
  const [[ex, ey], [bx, by]] = [l.end, l.body];
  return l.bend ? `M${ex} ${ey + d}L${l.bend[0] + d} ${l.bend[1] + d}L${bx + d} ${by}` : `M${ex} ${ey + d}L${bx} ${by + d}`;
}

const HEX = /^#[0-9a-f]{6}$/i;
/** The largest coordinate of a corner of a custom outline, as the engine checks it. */
export const OUTLINE_MAX = 480;

/** True for an outline that draws a custom shape: 3 to 32 corners in 0 to 480. A file can hold a bad one. */
export const isOutline = (o: readonly Point[] | undefined): o is Point[] =>
  !!o && o.length >= 3 && o.length <= 32 && o.every((p) => p.length === 2 && p.every((v) => Number.isFinite(v) && v >= 0 && v <= OUTLINE_MAX));

/** The layout of a symbol. The pin places are the `end` of the leads, by "i:<key>" and "o:<key>". */
export function symbolLayout(symbol: Symbol, name: string, inputs: readonly Pin[], outputs: readonly Pin[], lanes: Lanes = () => 0): SymbolLayout {
  const layout = shapeLayout(symbol, name, inputs, outputs, lanes);
  return symbol.color && HEX.test(symbol.color) ? { ...layout, color: symbol.color } : layout;
}

/** A rectangle for a new custom outline, with room for the pin names and the label. */
export function startOutline(symbol: Symbol, name: string, inputs: readonly Pin[], outputs: readonly Pin[], lanes: Lanes = () => 0): Point[] {
  const label = symbol.label || name.toUpperCase();
  const [inRooms, outRooms] = [inputs.map((p) => roomOf(lanes(`i:${p.key}`))), outputs.map((p) => roomOf(lanes(`o:${p.key}`)))];
  const w = Math.min(OUTLINE_MAX, Math.max(60, widest(inputs.map(nameOf)) + widest(outputs.map(nameOf)) + label.length * LABEL_CHAR + 24));
  const h = Math.min(OUTLINE_MAX, Math.max(sum(inRooms), sum(outRooms), 2 * PITCH) + 16);
  return [[0, 0], [w, 0], [w, h], [0, h]];
}

function shapeLayout(symbol: Symbol, name: string, inputs: readonly Pin[], outputs: readonly Pin[], lanes: Lanes): SymbolLayout {
  const label = symbol.label || name.toUpperCase();
  const n: Count = (side, p) => lanes(`${side}:${p.key}`);
  switch (symbol.shape) {
    case 'mux':
    case 'alu':
      return block(symbol.shape, label, inputs, outputs, n);
    case 'adder':
      return adder(label, inputs, outputs, n);
    case 'chip':
    case 'code':
      return chip(label, inputs, outputs, n);
    case 'bus':
      return bus(label, inputs, outputs, n);
    case 'custom':
      return isOutline(symbol.outline) ? custom(label, symbol.outline, inputs, outputs, n) : chip(label, inputs, outputs, n);
    default:
      return gate(symbol.shape, inputs, outputs, n);
  }
}

/** A logic gate with the distinctive shape of IEEE 91. */
function gate(shape: Shape, inputs: readonly Pin[], outputs: readonly Pin[], n: Count): SymbolLayout {
  const one = shape === 'not' || shape === 'buffer';
  const bubble = shape === 'nand' || shape === 'nor' || shape === 'xnor' || shape === 'not';
  const or = shape === 'or' || shape === 'nor' || shape === 'xor' || shape === 'xnor';
  const xor = shape === 'xor' || shape === 'xnor';
  const [inN, outN] = [inputs.map((p) => n('i', p)), outputs.map((p) => n('o', p))];
  const height = Math.max(40, sum(inN.map(roomOf)), sum(outN.map(roomOf)));
  const mid = height / 2;
  // The body is as tall as the widest bus lead, so that its lanes meet the body.
  const tall = Math.max(one ? 28 : Math.min(height, 60), 2 * Math.max(0, ...inN.map(halfOf), ...outN.map(halfOf)) + 8);
  const [y0, y1] = [mid - tall / 2, mid + tall / 2];
  const long = one ? Math.max(26, Math.round(tall * 0.9)) : Math.round(tall * 1.2);
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
  const ins = stack(inN.map(roomOf), mid);
  // An input past the body ends on a line that extends the back, as on a wide gate of a schematic.
  const [first, last] = [Math.min(y0, ...ins.map((y, i) => y - halfOf(inN[i]))), Math.max(y1, ...ins.map((y, i) => y + halfOf(inN[i])))];
  if (first < y0 || last > y1) parts.push({ d: `M${x0} ${first}V${last}`, fill: 'none' });
  // The point on the curved back of an OR shape: a quadratic curve whose y is linear in t.
  const backAt = (y: number) => {
    const t = Math.min(1, Math.max(0, (y - y0) / tall));
    return or ? x0 + 2 * t * (1 - t) * curve : back;
  };
  // The lanes of a bus go to the point of the back nearest the middle, so that no lane stops short of the body.
  const reach = (y: number, half: number) => backAt(Math.min(Math.max(mid, y - half), y + half));
  const leads = [
    ...inputs.map((p, i) => lead('i', p, [0, ins[i]], [reach(ins[i], halfOf(inN[i])), ins[i]], inN[i])),
    ...stack(outN.map(roomOf), mid).map((y, i) => lead('o', outputs[i], [out + LEAD, y], [outN[i] > 1 ? (one ? back + 2 : back + long / 2) : out, y], outN[i])),
  ];
  return { width: out + LEAD, height, leads, parts, texts: [], lit: outputs[0] && `o:${outputs[0].key}` };
}

/**
 * The control inputs on the bottom edge, from (x0, tall) to (x0 + long, tall - slant). Each lead
 * comes from a pin place on the left edge, so its wire meets it as a left pin (15.9). A lead further
 * right is lower, so the leads do not cross. Returns the bottom of the leads.
 */
function controlLeads(controls: readonly Pin[], n: Count, x0: number, long: number, tall: number, slant: number, leads: Lead[], texts: Text[]): number {
  const edge = (x: number) => tall - (slant * (x - x0)) / long;
  let [y, half] = [tall + 6, 0];
  controls.forEach((p, i) => {
    const lanes = n('i', p);
    const next = halfOf(lanes);
    y = i === 0 ? y + next : y + Math.max(12, half + next + 10);
    half = next;
    const x = Math.round(x0 + (long * (i + 1)) / (controls.length + 1));
    leads.push({ ...lead('i', p, [0, y], [x, edge(x + half)], lanes), bend: [x, y] });
    texts.push({ at: [x, edge(x) - 5], text: nameOf(p), anchor: 'middle', kind: 'pin' });
  });
  return controls.length > 0 ? y + half + 6 : tall;
}

/** A multiplexer (a trapezoid) or an ALU (a chevron with a notch between its two operands). */
function block(shape: 'mux' | 'alu', label: string, inputs: readonly Pin[], outputs: readonly Pin[], n: Count): SymbolLayout {
  const controls = inputs.filter(isControl);
  const data = inputs.filter((p) => !isControl(p));
  const alu = shape === 'alu';
  const upper = Math.ceil(data.length / 2);
  const inRooms = data.map((p) => roomOf(n('i', p)));
  // The ALU operands go above and below the notch.
  if (alu) inRooms.splice(upper, 0, PITCH);
  const outRooms = outputs.map((p) => roomOf(n('o', p)));
  const inner = alu ? Math.max(sum(inRooms), sum(outRooms) + 2 * PITCH, 3 * PITCH) : Math.max(sum(inRooms), sum(outRooms) + PITCH, 2 * PITCH);
  // The right edge of an ALU is 56% of its height, between the two slants.
  const tall = Math.max(inner + 16, alu ? Math.ceil(sum(outRooms) / 0.56) : 0);
  const mid = tall / 2;
  const slant = alu ? tall * 0.22 : Math.min(16, tall / 4);
  const [inW, outW] = [widest(data.map(nameOf)), widest(outputs.map(nameOf))];
  const room = Math.max(24, ...controls.map((p) => 2 * halfOf(n('i', p)) + 12));
  const long = Math.max(alu ? 64 : 52, inW + outW + label.length * LABEL_CHAR + 24, (controls.length + 1) * room - 8);
  const [x0, x1] = [LEAD, LEAD + long];
  const places = stack(inRooms, mid);
  const ins = alu ? places.filter((_, i) => i !== upper) : places;
  const notch = alu ? `V${places[upper] + 7}L${x0 + 10} ${places[upper]}L${x0} ${places[upper] - 7}` : '';
  const parts: Part[] = [{ d: `M${x0} 0L${x1} ${slant}V${tall - slant}L${x0} ${tall}${notch}Z`, fill: 'body' }];
  const outs = stack(outRooms, mid);
  const leads = [...data.map((p, i) => lead('i', p, [0, ins[i]], [x0, ins[i]], n('i', p))), ...outputs.map((p, i) => lead('o', p, [x1 + LEAD, outs[i]], [x1, outs[i]], n('o', p)))];
  const texts = [
    ...data.map((p, i) => pinText([x0 + 5, ins[i]], nameOf(p), 'start')),
    ...outputs.map((p, i) => pinText([x1 - 5, outs[i]], nameOf(p), 'end')),
    { at: [middle(x0, long, inW, outW), mid + 4] as Point, text: label, anchor: 'middle' as const, kind: 'label' as const },
  ];
  const height = controlLeads(controls, n, x0, long, tall, slant, leads, texts);
  return { width: x1 + LEAD, height, leads, parts, texts };
}

/** An adder: a block with a sigma sign and its label under it. */
function adder(label: string, inputs: readonly Pin[], outputs: readonly Pin[], n: Count): SymbolLayout {
  const [inRooms, outRooms] = [inputs.map((p) => roomOf(n('i', p))), outputs.map((p) => roomOf(n('o', p)))];
  const tall = Math.max(sum(inRooms), sum(outRooms), 2 * PITCH) + 20;
  const mid = tall / 2;
  const [inW, outW] = [widest(inputs.map(nameOf)), widest(outputs.map(nameOf))];
  const long = inW + outW + Math.max(24, label.length * CHAR) + 26;
  const [x0, x1] = [LEAD, LEAD + long];
  const [cx, cy] = [middle(x0, long, inW, outW), mid - 6];
  const parts: Part[] = [
    { d: `M${x0 + 4} 0H${x1 - 4}Q${x1} 0 ${x1} 4V${tall - 4}Q${x1} ${tall} ${x1 - 4} ${tall}H${x0 + 4}Q${x0} ${tall} ${x0} ${tall - 4}V4Q${x0} 0 ${x0 + 4} 0Z`, fill: 'body' },
    { d: `M${cx + 7} ${cy - 9}H${cx - 7}L${cx + 1} ${cy}L${cx - 7} ${cy + 9}H${cx + 7}`, fill: 'none' },
  ];
  const [ins, outs] = [stack(inRooms, mid), stack(outRooms, mid)];
  const leads = [...inputs.map((p, i) => lead('i', p, [0, ins[i]], [x0, ins[i]], n('i', p))), ...outputs.map((p, i) => lead('o', p, [x1 + LEAD, outs[i]], [x1, outs[i]], n('o', p)))];
  const texts = [...inputs.map((p, i) => pinText([x0 + 5, ins[i]], nameOf(p), 'start')), ...outputs.map((p, i) => pinText([x1 - 5, outs[i]], nameOf(p), 'end')), pinText([cx, cy + 20], label, 'middle')];
  return { width: x1 + LEAD, height: tall, leads, parts, texts };
}

/** A DIP chip: a black body with a notch, a pin 1 mark, metal legs, the pin names and the label. */
function chip(label: string, inputs: readonly Pin[], outputs: readonly Pin[], n: Count): SymbolLayout {
  const [inRooms, outRooms] = [inputs.map((p) => roomOf(n('i', p))), outputs.map((p) => roomOf(n('o', p)))];
  const inner = Math.max(sum(inRooms), sum(outRooms), PITCH);
  const top = 14;
  const tall = top + inner + 6;
  const inW = Math.max(0, ...inputs.map((p) => nameOf(p).length * CHAR + (isClock(p) ? 9 : 0)));
  const outW = widest(outputs.map(nameOf));
  const long = Math.max(72, inW + outW + label.length * LABEL_CHAR + 36);
  const [x0, x1] = [LEAD, LEAD + long];
  const cx = x0 + long / 2;
  // A side with fewer pins is in the middle, on the rows of the other side.
  const at = (rooms: number[]) => stack(rooms, top + Math.floor((inner - sum(rooms)) / 2 / PITCH) * PITCH + sum(rooms) / 2);
  const parts: Part[] = [
    { d: `M${x0} 3Q${x0} 0 ${x0 + 3} 0H${cx - 6}A6 6 0 0 0 ${cx + 6} 0H${x1 - 3}Q${x1} 0 ${x1} 3V${tall - 3}Q${x1} ${tall} ${x1 - 3} ${tall}H${x0 + 3}Q${x0} ${tall} ${x0} ${tall - 3}Z`, fill: 'chip' },
    { d: `M${x0 + 5} 7a2 2 0 1 0 4 0a2 2 0 1 0 -4 0`, fill: 'mark' },
  ];
  const leads: Lead[] = [];
  const texts: Text[] = [];
  const [ins, outs] = [at(inRooms), at(outRooms)];
  inputs.forEach((p, i) => {
    const y = ins[i];
    leads.push(lead('i', p, [0, y], [x0, y], n('i', p), true));
    const clock = isClock(p);
    if (clock) parts.push({ d: `M${x0} ${y - 5}L${x0 + 7} ${y}L${x0} ${y + 5}`, fill: 'none' });
    texts.push(pinText([x0 + 5 + (clock ? 9 : 0), y], nameOf(p), 'start'));
  });
  outputs.forEach((p, i) => {
    leads.push(lead('o', p, [x1 + LEAD, outs[i]], [x1, outs[i]], n('o', p), true));
    texts.push(pinText([x1 - 5, outs[i]], nameOf(p), 'end'));
  });
  texts.push({ at: [middle(x0, long, inW, outW), top + inner / 2 + 4], text: label, anchor: 'middle', kind: 'label' });
  return { width: x1 + LEAD, height: tall, leads, parts, texts };
}

/** The digits of pin names that share a prefix, for example 0 to 7 of b0 to b7, or else the names. */
function tags(pins: readonly Pin[]): string[] {
  const names = pins.map(nameOf);
  const found = names.map((n) => /^(.*?)(\d+)$/.exec(n));
  return found.every((m) => m && m[1] === found[0]![1]) ? found.map((m) => m![2]) : names;
}

/** A bus bar: the lines of the side with more pins tap the bar, and the other side joins its middle. */
function bus(label: string, inputs: readonly Pin[], outputs: readonly Pin[], n: Count): SymbolLayout {
  const split = outputs.length >= inputs.length;
  const [many, few] = split ? [outputs, inputs] : [inputs, outputs];
  const [manySide, fewSide] = split ? (['o', 'i'] as const) : (['i', 'o'] as const);
  const [manyN, fewN] = [many.map((p) => n(manySide, p)), few.map((p) => n(fewSide, p))];
  const inner = Math.max(sum(manyN.map(roomOf)), sum(fewN.map(roomOf)), PITCH);
  const top = 18;
  const height = top + inner;
  const mid = top + inner / 2;
  const names = tags(many);
  // The stub keeps room for the caption icons between the order pins (15.3).
  const stub = Math.max(36, widest(names) + 14);
  const bar = split ? LEAD + 10 : stub;
  const width = bar + (split ? stub : LEAD + 10);
  const [manyX, fewX] = split ? [width, 0] : [0, width];
  const ys = stack(manyN.map(roomOf), mid);
  const parts: Part[] = [{ d: `M${bar} ${top + 2}V${height - 2}`, fill: 'none', width: 6 }];
  const leads = [...many.map((p, i) => lead(manySide, p, [manyX, ys[i]], [bar, ys[i]], manyN[i])), ...stack(fewN.map(roomOf), mid).map((y, i) => lead(fewSide, few[i], [fewX, y], [bar, y], fewN[i]))];
  const texts = [...many.map((_, i) => ({ at: [split ? bar + 6 : bar - 6, ys[i] - halfOf(manyN[i]) - 3] as Point, text: names[i], anchor: split ? ('start' as const) : ('end' as const), kind: 'pin' as const })), { at: [bar, top - 6] as Point, text: label, anchor: 'middle' as const, kind: 'label' as const }];
  return { width, height, leads, parts, texts };
}

/**
 * A custom outline (4.3) at (LEAD, top). The inputs come from the left and the outputs from the
 * right, and each lead goes to the outline. The pins are on the middle of the outline, and `top`
 * gives them room when they are taller than it.
 */
function custom(label: string, outline: readonly Point[], inputs: readonly Pin[], outputs: readonly Pin[], n: Count): SymbolLayout {
  const [xs, ys] = [outline.map((p) => p[0]), outline.map((p) => p[1])];
  const [minY, maxX, maxY] = [Math.min(...ys), Math.max(...xs), Math.max(...ys)];
  const [inRooms, outRooms] = [inputs.map((p) => roomOf(n('i', p))), outputs.map((p) => roomOf(n('o', p)))];
  const half = Math.max(sum(inRooms), sum(outRooms)) / 2;
  const center = (minY + maxY) / 2;
  const top = Math.max(0, half - center);
  const mid = top + center;
  const pts = outline.map(([x, y]): Point => [LEAD + x, top + y]);
  // The x of each crossing of the outline and the horizontal line at y.
  const cross = (y: number) =>
    pts.flatMap(([ax, ay], i) => {
      const [bx, by] = pts[(i + 1) % pts.length];
      return ay <= y !== by <= y ? [ax + ((y - ay) * (bx - ax)) / (by - ay)] : [];
    });
  const x1 = LEAD + maxX;
  const leads: Lead[] = [];
  const texts: Text[] = [];
  stack(inRooms, mid).forEach((y, i) => {
    const hits = cross(y);
    const x = hits.length > 0 ? Math.min(...hits) : LEAD;
    leads.push(lead('i', inputs[i], [0, y], [x, y], n('i', inputs[i])));
    texts.push(pinText([x + 5, y], nameOf(inputs[i]), 'start'));
  });
  stack(outRooms, mid).forEach((y, i) => {
    const hits = cross(y);
    const x = hits.length > 0 ? Math.max(...hits) : x1;
    leads.push(lead('o', outputs[i], [x1 + LEAD, y], [x, y], n('o', outputs[i])));
    texts.push(pinText([x - 5, y], nameOf(outputs[i]), 'end'));
  });
  const xMid = (LEAD + Math.min(...xs) + x1) / 2;
  texts.push({ at: [xMid, mid + 4], text: label, anchor: 'middle', kind: 'label' });
  const parts: Part[] = [{ d: `M${pts.map((p) => p.join(' ')).join('L')}Z`, fill: 'body' }];
  return { width: x1 + LEAD, height: Math.max(top + maxY, mid + half), leads, parts, texts, origin: [LEAD, top] };
}

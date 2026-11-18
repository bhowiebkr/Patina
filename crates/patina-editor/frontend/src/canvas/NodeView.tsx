// A node of the graph: a neutral header with the state icon, and a row for each pin (spec 15.3).

import { createContext, memo, useContext, type CSSProperties } from 'react';
import { Handle, Position, useStore, type Node, type NodeProps } from '@xyflow/react';
import { duration, errorText, folderName, nodeProblems, openType, reportText, type Diagnostic, type Domain, type Edit, type NodeEntry, type NodeInfo, type NodeLog, type NodeTime, type Pin, type Report, type Value, type View } from '../api';
import type { Socket } from './bus';
import { BusSocket, socketBar, socketHeight, socketPin } from './BusSocket';
import { FAR_ZOOM } from '../wires/gl';
import { ExpandButton, NodeGui } from '../controls/NodeGui';
import { Thumbnail } from '../controls/ValueWidgets';
import { outputTypeOf, socketKey } from './palette';
import { FRAME_BOTTOM, FRAME_RIGHT } from './frames';
import { freeLayout, nodeWidth } from '../controls/gui';
import { DomainIcon, KindIcon } from '../controls/icons';
import type { Lost } from './lostPins';

export type NodeState = 'none' | 'ready' | 'compiling' | 'compile-error' | 'run-error' | 'stale' | 'skipped' | 'unresolved';

/** The last result of the engine for the open project. */
export interface RunState {
  /** 'lost' is the end of the node process after a run (16.1). The build stays. */
  phase: 'none' | 'building' | 'loaded' | 'ran' | 'failed' | 'lost';
  builtMs: number | null;
  /** The time of the last build that compiled. */
  lastBuildMs: number | null;
  /** The time of the last build of the base and of the slot that compiled (7.3). */
  baseMs: number | null;
  slotMs: number | null;
  runMs: number | null;
  ok: boolean;
  reports: Report[];
  /** Each live value by output, "<node id>.<output key>" (15.9). */
  values: ReadonlyMap<string, Value>;
  /** The compile errors of the last build (15.5). */
  errors: Diagnostic[];
  text: string;
  /** True after an edit while the graph is paused: the result is for an old graph. */
  stale: boolean;
  /** Each node that did not run in the last run, by ID (15.2, 15.3). Its values are old. */
  idle: ReadonlyMap<string, 'skipped' | 'stopped'>;
  /** The run time, the start and the thread of each node call in the last run, by node ID (15.7). */
  times: ReadonlyMap<string, NodeTime>;
  /** The log lines of the nodes in the last run (15.7). */
  logs: NodeLog[];
  /** The number of frames of the frame run after the last batch run (9.1). */
  frames: number;
  /** True when the last run has frame nodes, so **Step** can run one frame (9.1). */
  framed: boolean;
  /** The run errors of the event runs since the last run (9.4). `reports` has them too. */
  eventReports: Report[];
  /** True for clusters in bake mode with no slot (7.3). */
  fused: boolean;
  /** The node folder in the slot (7.1), or null. */
  slot: string | null;
  /** The layers of the current build: the base, and the node folder of the slot library (7.3). */
  building: { base: boolean; slot: string | null };
}

export const NO_RUN: RunState = {
  phase: 'none',
  builtMs: null,
  lastBuildMs: null,
  baseMs: null,
  slotMs: null,
  runMs: null,
  ok: true,
  reports: [],
  values: new Map(),
  errors: [],
  text: '',
  stale: false,
  idle: new Map(),
  times: new Map(),
  logs: [],
  frames: 0,
  framed: false,
  eventReports: [],
  fused: false,
  slot: null,
  building: { base: true, slot: null },
};

/** The values and idle nodes of one use of a subgraph, with the IDs inside the use (6.4, 15.9). */
export function scopeRun(run: RunState, scope: string): RunState {
  if (!scope) return run;
  const p = `${scope}/`;
  const inner = <T,>(map: ReadonlyMap<string, T>) => [...map].filter(([k]) => k.startsWith(p)).map(([k, v]) => [k.slice(p.length), v] as const);
  const values = new Map(inner(run.values).map(([k, v]) => [k, { ...v, from: k }]));
  // The outputs of the use go to the public output pins, as "out:<key>.pin".
  for (const [k, v] of run.values) {
    if (k.startsWith(`${scope}.`)) values.set(`out:${k.slice(scope.length + 1)}.pin`, { ...v, from: `out:${k.slice(scope.length + 1)}.pin` });
  }
  return { ...run, values, idle: new Map(inner(run.idle)) };
}

/** The compile errors in the folder of a node. */
export function nodeErrors(run: RunState, name: string): Diagnostic[] {
  return run.errors.filter((e) => e.file?.startsWith(`nodes/${name}/`));
}

export interface NodeData extends Record<string, unknown> {
  entry: NodeEntry;
  info: NodeInfo | null;
  wired: string[];
  /** The wire ends that the node has no pin for. Each gets a row, so its wire shows (15.9). */
  lost?: Lost;
  state: NodeState;
  message: string;
  editable: boolean;
  /** The run time of the node in the last run, in milliseconds (15.7). */
  time?: number;
  /** The widget lines of the node GUI in the node body size (4.3). */
  gui?: string;
  /** True when the node runs in the slot library (7.3). */
  slot?: boolean;
  /** The clock domain of the node use (9.1). */
  domain?: Domain;
  /** The bus sockets (15.9) by "i:<input key>" or "o:<output key>". */
  sockets?: Readonly<Record<string, Socket>>;
  /** True for a subgraph use that shows its inner nodes in a frame (15.3). */
  open?: boolean;
  /** True for an inner node of such a frame. It has no edits and no header buttons. */
  inner?: boolean;
  /** The thumbnail of the first output that has one (5.3), with the name of the output. */
  thumb?: { text: string; stale: boolean; label: string };
}

export type PatinaNode = Node<NodeData, 'patina'>;

/** A public pin of the open subgraph (6.4): an input gives a value to the inner nodes. */
/** `socket` is the bus socket of a public pin whose kind gives lanes (15.9). */
export type PortNode = Node<{ name: string; ty: string; input: boolean; value?: string; testing: boolean; socket?: Socket }, 'port'>;

/** The ID of the use that runs the open subgraph alone. It is `TEST` in patina-engine (15.9). */
export const TEST_USE = '7e57000000000000';

/** Switches the public input with this key while the subgraph runs alone (15.9). */
export const PortTest = createContext<((key: string) => void) | null>(null);

/** Applies an edit from a control in a node body, for example a check box of a `bool` input. */
export const NodeEdit = createContext<(edit: Edit) => void>(() => {});

/** Fires a `trigger` node of the live run (9.4). Null while the graph is paused. */
export const FireTrigger = createContext<((id: string) => void) | null>(null);

/** Opens a subgraph through one of its uses, as a double-click on the use does (15.9). */
export const OpenUse = createContext<((graph: string, use: string) => void) | null>(null);

/** Shows or hides the inner nodes of a subgraph use (15.3). */
export const FrameToggle = createContext<((use: string) => void) | null>(null);

/** The handle ID of the order sockets in the header of a node (9.2). No pin key starts with "@". */
export const ORDER = '@order';

export const HEADER = 32;
export const ROW = 24;
const PAD = 6;
/** The height of the caption above a free node (canvas.css). */
const CAPTION = 18;
/** The padding on each side of the wrapper of a graph node, which holds its pins (canvas.css). */
export const GUTTER = 8;
/** The gutter of a React Flow node. The graph position is the left edge of the body, not of the wrapper. */
export const gutter = (n: { type?: string }) => (n.type === 'patina' ? GUTTER : 0);

export const STATE_NAMES: Record<NodeState, string> = {
  none: 'Not run',
  ready: 'Ready',
  compiling: 'Compiling',
  'compile-error': 'Compile error',
  'run-error': 'Run error',
  stale: 'Stale',
  skipped: 'Skipped',
  unresolved: 'Unresolved',
};

export function nodeState(entry: NodeEntry, info: NodeInfo | null, view: View, run: RunState): [NodeState, string] {
  if (!info) return ['unresolved', entry.subgraph !== undefined ? `The subgraph ${entry.subgraph} is missing.` : `The folder ${entry.folder ?? ''} is missing.`];
  if (info.error) return ['compile-error', info.error];
  const problems = nodeProblems(view.problems, entry.id);
  if (problems.length > 0) return [problems.every(openType) ? 'unresolved' : 'compile-error', problems.join('\n')];
  if (run.phase === 'building') return ['compiling', ''];
  const errors = info.subgraph ? [] : nodeErrors(run, info.name);
  if (errors.length > 0) return ['compile-error', errors.map((e) => `line ${e.start[0]}: ${errorText(e)}`).join('\n')];
  if (run.phase === 'failed' || run.phase === 'lost') return ['stale', ''];
  if (run.phase !== 'ran') return ['none', ''];
  // A node of a subgraph use has the ID `<use ID>/<node ID>` (6.4).
  const reports = run.reports.filter((r) => r.id === entry.id || r.id.startsWith(`${entry.id}/`));
  if (reports.length > 0) {
    return ['run-error', reports.map(reportText).join('\n')];
  }
  const idle = run.idle.get(entry.id);
  if (idle === 'skipped') return ['skipped', 'Skipped: an input has no value.'];
  return [run.stale || idle === 'stopped' ? 'stale' : 'ready', ''];
}

/** The node info of an entry, or null when its folder, subgraph or built-in node is missing. */
export function infoOf(entry: NodeEntry, nodes: NodeInfo[]): NodeInfo | null {
  if (entry.subgraph !== undefined) return nodes.find((n) => n.subgraph && n.name === entry.subgraph) ?? null;
  const builtin = entry.builtin !== undefined;
  if (!builtin && !entry.folder?.startsWith('nodes/')) return null;
  const name = folderName(entry);
  return nodes.find((n) => n.name === name && n.builtin === builtin && !n.subgraph) ?? null;
}

/** The Rust type of output `from`, "<node id>.<output key>", or undefined when its node does not read. */
export function outputType(view: View, from: string): string | undefined {
  const pins = (id: string) => {
    const entry = view.graph.node?.find((n) => n.id === id);
    return entry && infoOf(entry, view.nodes);
  };
  return outputTypeOf(from, pins, (to) => view.graph.wire?.find((w) => w.to === to)?.from);
}

function StateIcon({ state }: { state: NodeState }) {
  const name = STATE_NAMES[state];
  let icon;
  switch (state) {
    case 'ready':
      icon = <span className="dot ready" />;
      break;
    case 'compiling':
      icon = <span className="dot" />;
      break;
    case 'compile-error':
    case 'run-error':
      icon = (
        <svg width="14" height="14" viewBox="0 0 14 14" aria-hidden="true">
          <circle cx="7" cy="7" r="6.5" fill="var(--error)" />
          <rect x="6.2" y="3" width="1.6" height="5" fill="var(--app)" />
          <rect x="6.2" y="9.4" width="1.6" height="1.6" fill="var(--app)" />
        </svg>
      );
      break;
    case 'stale':
      icon = (
        <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true">
          <circle cx="6" cy="6" r="4.5" fill="none" stroke="var(--subtle)" strokeWidth="1.5" />
        </svg>
      );
      break;
    case 'skipped':
      icon = (
        <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true">
          <circle cx="6" cy="6" r="4.5" fill="none" stroke="var(--subtle)" strokeWidth="1.5" />
          <path d="M3 9 9 3" stroke="var(--subtle)" strokeWidth="1.5" />
        </svg>
      );
      break;
    case 'unresolved':
      icon = (
        <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true">
          <circle cx="6" cy="6" r="4.5" fill="none" stroke="var(--disabled)" strokeWidth="1.5" strokeDasharray="2 2" />
        </svg>
      );
      break;
    default:
      return null;
  }
  return (
    <span className={`state-icon ${state}`} role="img" aria-label={name} title={name}>
      {icon}
    </span>
  );
}

function NodeViewImpl({ id, data, selected }: NodeProps<PatinaNode>) {
  const { entry, info, wired, lost, state, message, editable, time, gui, slot, sockets, domain, open, inner, thumb } = data;
  const inputs = lost ? [...(info?.inputs ?? []), ...lost.inputs.map(lostPin)] : (info?.inputs ?? []);
  const outputs = lost ? [...(info?.outputs ?? []), ...lost.outputs.map(lostPin)] : (info?.outputs ?? []);
  const lostIn = (pin: Pin | undefined) => !!pin && !!lost?.inputs.includes(pin.key);
  const lostOut = (pin: Pin | undefined) => !!pin && !!lost?.outputs.includes(pin.key);
  const rows = Math.max(inputs.length, outputs.length);
  const socket = (side: 'i' | 'o', pin: Pin | undefined) => (pin ? sockets?.[`${side}:${pin.key}`] : undefined);
  // A row with a bus socket is as tall as its contacts (15.9).
  const tops = [HEADER + PAD];
  for (let i = 0; i < rows; i++) {
    const [a, b] = [socket('i', inputs[i]), socket('o', outputs[i])];
    tops.push(tops[i] + Math.max(ROW, a ? socketHeight(a.bits.length) : 0, b ? socketHeight(b.bits.length) : 0));
  }
  const pinTop = (i: number, s: Socket | undefined) => (s ? socketPin(tops[i], s.bits.length) : tops[i] + ROW / 2);
  // At a far zoom, the rows keep their size but have no text and the GUI does not draw, so the page has fewer elements (15.1).
  const far = useStore((s) => s.transform[2] < FAR_ZOOM);
  const edit = useContext(NodeEdit);
  const toggle = useContext(FrameToggle);
  // The right and bottom edges of the inner nodes, as one number so that a pan does not render the frame.
  const size = useStore((s) => {
    if (!open) return 0;
    let [r, b] = [0, 0];
    for (const n of s.nodeLookup.values()) {
      if (n.parentId !== id) continue;
      r = Math.max(r, n.position.x + (n.measured.width ?? 0) - 2 * gutter(n));
      b = Math.max(b, n.position.y + (n.measured.height ?? 0));
    }
    return Math.ceil(r) * 65536 + Math.ceil(b);
  });
  const classes = ['patina-node', state];
  if (open) classes.push('frame');
  if (selected) classes.push('selected');
  if (far) classes.push('far');
  if (info?.color) classes.push('colored');
  if (info?.free && gui !== undefined && !open) {
    // A free layout (4.3): the GUI is the whole node, with a caption above it and the pins at their places.
    const layout = freeLayout(gui);
    const width = nodeWidth(gui, true);
    // shortcut: a wire leaves each pin to the left or the right, also from the middle of the shape; add a side to `Ui::pin` when a node needs it.
    const place = (pin: Pin, i: number, output: boolean): [number, number] => {
      const n = output ? outputs.length : inputs.length;
      return layout.pins.get(pin.key) ?? [output ? width : 0, layout.height > 0 ? (layout.height * (i + 1)) / (n + 1) : ROW / 2 + i * ROW];
    };
    const handle = (pin: Pin, i: number, output: boolean) => {
      const [x, y] = place(pin, i, output);
      const gone = output ? lostOut(pin) : lostIn(pin);
      return <PinHandle key={`${output ? 'o' : 'i'}-${pin.key}`} pin={pin} generics={info.generics} output={output} editable={editable} lost={gone} top={y} left={GUTTER + x} socket={undefined} />;
    };
    return (
      <div className={[...classes, 'free'].join(' ')} style={{ width }}>
        <div className="free-caption" title={info.doc ?? undefined}>
          <span className="title">{folderName(entry)}</span>
          {domain && <DomainIcon domain={domain} />}
          <StateIcon state={state} />
        </div>
        <NodeGui id={entry.id} mode={0} tree={gui} disabled={!editable} />
        {message && <div className="patina-node-message">{message}</div>}
        {inputs.map((pin, i) => handle(pin, i, false))}
        {outputs.map((pin, i) => handle(pin, i, true))}
        <Handle id={ORDER} type="target" position={Position.Left} isConnectable={editable} className="order-pin" style={{ top: -CAPTION / 2 }} title="Order wire from a node that runs before this node" />
        <Handle id={ORDER} type="source" position={Position.Right} isConnectable={editable} className="order-pin" style={{ top: -CAPTION / 2 }} title="Drag to a node that must run after this node" />
      </div>
    );
  }
  const style = { width: gui === undefined ? undefined : nodeWidth(gui), minWidth: open ? Math.floor(size / 65536) + FRAME_RIGHT : undefined, '--head': info?.color ?? undefined } as CSSProperties;
  const use = entry.subgraph !== undefined && !inner;
  return (
    <div className={classes.join(' ')} style={style}>
      <div className="patina-node-header" title={info?.doc ?? undefined}>
        {info && <KindIcon kind={info.kind} />}
        <span className="title">{folderName(entry)}</span>
        {info?.full && !inner && <ExpandButton id={entry.id} />}
        {entry.builtin === 'trigger' && !inner && <FireButton id={entry.id} />}
        {use && <OpenButton graph={entry.subgraph!} use={entry.id} />}
        {use && toggle && <FrameButton open={!!open} onClick={() => toggle(entry.id)} />}
        {slot && (
          <span className="state-icon" role="img" title="In the slot" aria-label="In the slot">
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.4" aria-hidden="true">
              <rect x="1.5" y="3.5" width="11" height="7" rx="1.5" />
              <path d="M4.5 7h5" />
            </svg>
          </span>
        )}
        {domain && <DomainIcon domain={domain} />}
        <StateIcon state={state} />
      </div>
      <div className="patina-rows" style={{ height: tops[rows] - HEADER + PAD }}>
        {!far && Array.from({ length: rows }, (_, i) => {
          const [input, output] = [inputs[i], outputs[i]];
          const height = tops[i + 1] - tops[i];
          const row = { height, alignItems: height > ROW ? 'flex-start' : undefined };
          // A socket replaces the label of its pin.
          const [busIn, busOut] = [socket('i', input), socket('o', output)];
          if (busIn) return <div key={i} className="patina-row" style={row}><span />{!busOut && <PinLabel pin={output} lost={lostOut(output)} />}</div>;
          if (lostIn(input)) return <div key={i} className="patina-row" style={row}><PinLabel pin={input} lost />{!busOut && <PinLabel pin={output} lost={lostOut(output)} />}</div>;
          const free = input && !wired.includes(input.key);
          const text = free ? entry.params?.[input.key] : undefined;
          // An unwired bool input is a switch that the user clicks in the node, as in a logic simulator.
          const box = free && input.ty === 'bool';
          return (
            <div key={i} className="patina-row" style={row}>
              <span title={input && `${input.name ?? input.key}: ${input.ty}`}>
                {input && (input.name ?? input.key)}
                {box ? (
                  <input
                    type="checkbox"
                    className="nodrag pin-switch"
                    aria-label={input.name ?? input.key}
                    checked={text === 'true'}
                    disabled={!editable}
                    onChange={(e) => edit({ kind: 'SetParam', id: entry.id, key: input.key, text: String(e.currentTarget.checked) })}
                  />
                ) : (
                  text !== undefined && <span className="param-text"> {text}</span>
                )}
              </span>
              {busOut ? <span /> : <PinLabel pin={output} lost={lostOut(output)} />}
            </div>
          );
        })}
        {!far &&
          Array.from({ length: rows }, (_, i) =>
            (['i', 'o'] as const).map((side) => {
              const s = socket(side, side === 'i' ? inputs[i] : outputs[i]);
              return s && <BusSocket key={`${side}${i}`} socket={s} output={side === 'o'} top={tops[i] - HEADER} />;
            }),
          )}
      </div>
      {open && <div style={{ height: Math.max(0, (size % 65536) + FRAME_BOTTOM - tops[rows] - PAD) }} />}
      {thumb && <Thumbnail thumb={thumb.text} stale={thumb.stale} label={thumb.label} />}
      {gui !== undefined && <NodeGui id={entry.id} mode={0} tree={gui} disabled={!editable} />}
      {message && <div className={state === 'unresolved' || state === 'skipped' ? 'patina-node-message muted' : 'patina-node-message'}>{message}</div>}
      {time !== undefined && <div className="patina-node-time">{!far && `Run ${duration(time)}`}</div>}
      {inputs.map((pin, i) => (
        <PinHandle key={`i-${pin.key}`} pin={pin} generics={info?.generics} output={false} editable={editable} lost={lostIn(pin)} top={pinTop(i, socket('i', pin))} socket={socket('i', pin)} />
      ))}
      {outputs.map((pin, i) => (
        <PinHandle key={`o-${pin.key}`} pin={pin} generics={info?.generics} output editable={editable} lost={lostOut(pin)} top={pinTop(i, socket('o', pin))} socket={socket('o', pin)} />
      ))}
      {/* The inner ends of the pins of a frame: the inner wires start and end at the edge (15.3). */}
      {open && inputs.map((pin, i) => <FramePin key={`fi-${pin.key}`} id={`in:${pin.key}`} output={false} top={pinTop(i, socket('i', pin))} socket={socket('i', pin)} />)}
      {open && outputs.map((pin, i) => <FramePin key={`fo-${pin.key}`} id={`out:${pin.key}`} output top={pinTop(i, socket('o', pin))} socket={socket('o', pin)} />)}
      <Handle id={ORDER} type="target" position={Position.Left} isConnectable={editable} className="order-pin" style={{ top: HEADER / 2 }} title="Order wire from a node that runs before this node" />
      <Handle id={ORDER} type="source" position={Position.Right} isConnectable={editable} className="order-pin" style={{ top: HEADER / 2 }} title="Drag to a node that must run after this node" />
    </div>
  );
}

function FireButton({ id }: { id: string }) {
  const fire = useContext(FireTrigger);
  return (
    <button className="icon-btn expand nodrag" aria-label="Fire the trigger" title={fire ? 'Fire' : 'Run the graph, and open a subgraph through its use, to fire the trigger'} disabled={!fire} onClick={() => fire?.(id)}>
      <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinejoin="round" aria-hidden="true">
        <path d="M9.5 1.5 3.5 9h4l-1 5.5 6-7.5h-4z" />
      </svg>
    </button>
  );
}

/** The **Open subgraph** button of a use, in the node header and the Inspector (15.9). */
export function OpenButton({ graph, use }: { graph: string; use: string }) {
  const open = useContext(OpenUse);
  return (
    <button className="icon-btn expand nodrag open-subgraph" aria-label="Open subgraph" title={`Open subgraph ${graph} (double-click)`} disabled={!open} onClick={() => open?.(graph, use)}>
      <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinejoin="round" strokeLinecap="round" aria-hidden="true">
        <path d="M9 2.5h3.5a1 1 0 0 1 1 1v9a1 1 0 0 1-1 1H9M2 8h8M7 5l3 3-3 3" />
      </svg>
    </button>
  );
}

/** **Show inner nodes** and **Hide inner nodes** of a subgraph use (15.3). */
function FrameButton({ open, onClick }: { open: boolean; onClick: () => void }) {
  const name = open ? 'Hide inner nodes' : 'Show inner nodes';
  return (
    <button className="icon-btn expand nodrag frame-toggle" aria-label={name} aria-expanded={open} title={name} onClick={onClick}>
      <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinejoin="round" strokeLinecap="round" aria-hidden="true">
        <path d={open ? 'M4 10l4-4 4 4' : 'M4 6l4 4 4-4'} />
      </svg>
    </button>
  );
}

/** A pin of a frame on the inside: an input gives its value to the inner wires, an output takes it. */
function FramePin({ id, output, top, socket }: { id: string; output: boolean; top: number; socket: Socket | undefined }) {
  const bar = socket && socketBar(socket.bits.length);
  const style = bar ? { top, height: bar, marginTop: 1 - bar / 2 } : { top };
  return <Handle id={id} type={output ? 'target' : 'source'} position={output ? Position.Right : Position.Left} isConnectable={false} className={socket ? 'frame-pin bus-pin' : 'frame-pin'} style={style} />;
}

/** A row of a wire end that the node has no pin for, for example a removed public pin (6.4). */
const lostPin = (key: string): Pin => ({ key, ty: '' });
const lostTitle = (key: string) => `The node has no pin ${key}. Delete the wire, or give the node this pin again.`;

function PinLabel({ pin, lost = false }: { pin: Pin | undefined; lost?: boolean }) {
  if (pin && lost) return <span className="lost-pin" title={lostTitle(pin.key)}>{pin.key}</span>;
  return <span title={pin && `${pin.name ?? pin.key}: ${pin.ty}`}>{pin && (pin.name ?? pin.key)}</span>;
}

/** The pin of a row: a dot, or a bar as wide as the ribbon of a bus socket (15.9). */
function PinHandle({ pin, generics = [], output, editable, lost, top, left, socket }: { pin: Pin; generics?: string[]; output: boolean; editable: boolean; lost: boolean; top: number; left?: number; socket: Socket | undefined }) {
  const bar = socket && socketBar(socket.bits.length);
  const style = bar ? { top, height: bar, marginTop: 1 - bar / 2 } : { top, left };
  return (
    <Handle
      id={pin.key}
      type={output ? 'source' : 'target'}
      position={output ? Position.Right : Position.Left}
      isConnectable={editable && !lost}
      className={socket ? 'bus-pin' : lost ? 'lost-pin' : undefined}
      title={lost ? lostTitle(pin.key) : undefined}
      style={style}
      data-ty={socketKey(pin.ty, generics)}
    />
  );
}

export const NodeView = memo(NodeViewImpl);

export function PortView({ id, data }: NodeProps<PortNode>) {
  const { name, ty, input, value, testing, socket } = data;
  const toggle = useContext(PortTest);
  const far = useStore((s) => s.transform[2] < FAR_ZOOM);
  const bit = value === 'true' || value === 'false';
  const on = value === 'true';
  return (
    <div className={socket ? 'patina-node port bus' : 'patina-node port'} title={`${input ? 'Input' : 'Output'} ${name}: ${ty}`}>
      <div className="patina-node-header">
        <span className="title">{name}</span>
        {value !== undefined && !bit && <span className="port-value">{value}</span>}
      </div>
      {input && testing && bit && (
        <button
          className={`port-switch nodrag${on ? ' on' : ''}`}
          aria-pressed={on}
          aria-label={`Switch ${name}`}
          title={`${name} is ${on ? 'on' : 'off'}. Click to switch it.`}
          onClick={() => toggle?.(id.slice(id.indexOf(':') + 1))}
        />
      )}
      {!input && bit && <span className={`port-lamp${on ? ' on' : ''}`} role="img" aria-label={`${name} is ${on ? 'on' : 'off'}`} />}
      {socket && (
        <div className="patina-rows" style={{ height: socketHeight(socket.bits.length) + 2 * PAD }}>
          {!far && <BusSocket socket={socket} output={input} top={PAD} />}
        </div>
      )}
      <Handle
        id="pin"
        type={input ? 'source' : 'target'}
        position={input ? Position.Right : Position.Left}
        className={socket ? 'bus-pin' : undefined}
        style={socket ? { top: socketPin(HEADER + PAD, socket.bits.length), height: socketBar(socket.bits.length) } : { top: HEADER / 2 }}
        data-ty={socketKey(ty, ['?'])}
      />
    </div>
  );
}

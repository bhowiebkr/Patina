// A node of the graph: a neutral header with the state icon, and a row for each pin (spec 15.3).

import { createContext, memo, useContext } from 'react';
import { Handle, Position, useStore, type Node, type NodeProps } from '@xyflow/react';
import { duration, errorText, folderName, nodeProblems, reportText, type Diagnostic, type Edit, type NodeEntry, type NodeInfo, type NodeLog, type Report, type Value, type View } from './api';
import { FAR_ZOOM } from './gl';
import { ExpandButton, NodeGui } from './NodeGui';

export type NodeState = 'none' | 'ready' | 'compiling' | 'compile-error' | 'run-error' | 'stale' | 'skipped' | 'unresolved';

/** The last result of the engine for the open project. */
export interface RunState {
  phase: 'none' | 'building' | 'ran' | 'failed';
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
  /** The run time of each node call in the last run, in milliseconds by node ID (15.7). */
  times: ReadonlyMap<string, number>;
  /** The log lines of the nodes in the last run (15.7). */
  logs: NodeLog[];
  /** The number of frames of the frame run after the last batch run (9.1). */
  frames: number;
  /** True for clusters in bake mode with no slot (7.3). */
  fused: boolean;
  /** The node folder in the slot (7.1), or null. */
  slot: string | null;
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
  fused: false,
  slot: null,
};

/** True when the value of output `from`, "<node id>.<output key>", is from an old run. */
/** The values and idle nodes of one use of a subgraph, with the IDs inside the use (6.4, 15.9). */
export function scopeRun(run: RunState, scope: string): RunState {
  if (!scope) return run;
  const p = `${scope}/`;
  const inner = <T,>(map: ReadonlyMap<string, T>) => [...map].filter(([k]) => k.startsWith(p)).map(([k, v]) => [k.slice(p.length), v] as const);
  const values = new Map(inner(run.values).map(([k, v]) => [k, { ...v, from: k }]));
  return { ...run, values, idle: new Map(inner(run.idle)) };
}

export function valueStale(run: RunState, from: string): boolean {
  return run.stale || run.idle.has(from.slice(0, from.lastIndexOf('.')));
}

/** The compile errors in the folder of a node. */
export function nodeErrors(run: RunState, name: string): Diagnostic[] {
  return run.errors.filter((e) => e.file?.startsWith(`nodes/${name}/`));
}

export interface NodeData extends Record<string, unknown> {
  entry: NodeEntry;
  info: NodeInfo | null;
  wired: string[];
  state: NodeState;
  message: string;
  editable: boolean;
  /** The run time of the node in the last run, in milliseconds (15.7). */
  time?: number;
  /** The widget lines of the node GUI in the node body size (4.3). */
  gui?: string;
  /** True when the node runs in the slot library (7.3). */
  slot?: boolean;
}

export type PatinaNode = Node<NodeData, 'patina'>;

/** A public pin of the open subgraph (6.4): an input gives a value to the inner nodes. */
export type PortNode = Node<{ name: string; ty: string; input: boolean }, 'port'>;

/** Applies an edit from a control in a node body, for example a check box of a `bool` input. */
export const NodeEdit = createContext<(edit: Edit) => void>(() => {});

const HEADER = 32;
const ROW = 24;
const PAD = 6;

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
  if (problems.length > 0) return ['compile-error', problems.join('\n')];
  if (run.phase === 'building') return ['compiling', ''];
  const errors = info.subgraph ? [] : nodeErrors(run, info.name);
  if (errors.length > 0) return ['compile-error', errors.map((e) => `line ${e.start[0]}: ${errorText(e)}`).join('\n')];
  if (run.phase === 'failed') return ['stale', ''];
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
  const dot = from.lastIndexOf('.');
  const entry = view.graph.node?.find((n) => n.id === from.slice(0, dot));
  return (entry && infoOf(entry, view.nodes))?.outputs.find((p) => p.key === from.slice(dot + 1))?.ty;
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

function NodeViewImpl({ data, selected }: NodeProps<PatinaNode>) {
  const { entry, info, wired, state, message, editable, time, gui, slot } = data;
  const inputs = info?.inputs ?? [];
  const outputs = info?.outputs ?? [];
  const rows = Math.max(inputs.length, outputs.length);
  const pinTop = (i: number) => HEADER + PAD + i * ROW + ROW / 2;
  // At a far zoom, the rows keep their size but have no text, so the page has fewer elements (15.1).
  const far = useStore((s) => s.transform[2] < FAR_ZOOM);
  const edit = useContext(NodeEdit);
  const classes = ['patina-node', state];
  if (selected) classes.push('selected');
  if (far) classes.push('far');
  return (
    <div className={classes.join(' ')}>
      <div className="patina-node-header">
        <span className="title">{folderName(entry)}</span>
        {info?.full && <ExpandButton id={entry.id} />}
        {slot && (
          <span className="state-icon" role="img" title="In the slot" aria-label="In the slot">
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.4" aria-hidden="true">
              <rect x="1.5" y="3.5" width="11" height="7" rx="1.5" />
              <path d="M4.5 7h5" />
            </svg>
          </span>
        )}
        <StateIcon state={state} />
      </div>
      <div className="patina-rows" style={{ height: rows * ROW + 2 * PAD }}>
        {!far && Array.from({ length: rows }, (_, i) => {
          const [input, output] = [inputs[i], outputs[i]];
          const free = input && !wired.includes(input.key);
          const text = free ? entry.params?.[input.key] : undefined;
          // An unwired bool input is a switch that the user clicks in the node, as in a logic simulator.
          const box = free && input.ty === 'bool';
          return (
            <div key={i} className="patina-row">
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
              <span title={output && `${output.name ?? output.key}: ${output.ty}`}>{output && (output.name ?? output.key)}</span>
            </div>
          );
        })}
      </div>
      {gui !== undefined && !far && <NodeGui id={entry.id} mode={0} tree={gui} disabled={!editable} />}
      {message && <div className={state === 'unresolved' || state === 'skipped' ? 'patina-node-message muted' : 'patina-node-message'}>{message}</div>}
      {time !== undefined && <div className="patina-node-time">{!far && `Run ${duration(time)}`}</div>}
      {inputs.map(({ key }, i) => (
        <Handle key={`i-${key}`} id={key} type="target" position={Position.Left} isConnectable={editable} style={{ top: pinTop(i) }} />
      ))}
      {outputs.map(({ key }, i) => (
        <Handle key={`o-${key}`} id={key} type="source" position={Position.Right} isConnectable={editable} style={{ top: pinTop(i) }} />
      ))}
    </div>
  );
}

export const NodeView = memo(NodeViewImpl);

export function PortView({ data }: NodeProps<PortNode>) {
  const { name, ty, input } = data;
  return (
    <div className="patina-node port" title={`${input ? 'Input' : 'Output'} ${name}: ${ty}`}>
      <div className="patina-node-header">
        <span className="title">{name}</span>
      </div>
      <Handle id="pin" type={input ? 'source' : 'target'} position={input ? Position.Right : Position.Left} style={{ top: HEADER / 2 }} />
    </div>
  );
}

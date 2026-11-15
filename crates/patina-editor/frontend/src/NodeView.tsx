// A node of the graph: a neutral header with the state icon, and a row for each pin (spec 15.3).

import { memo } from 'react';
import { Handle, Position, type Node, type NodeProps } from '@xyflow/react';
import { folderName, nodeProblems, type NodeEntry, type NodeInfo, type Report, type View } from './api';

export type NodeState = 'none' | 'ready' | 'compiling' | 'compile-error' | 'run-error' | 'stale' | 'unresolved';

/** The last result of the engine for the open project. */
export interface RunState {
  phase: 'none' | 'building' | 'ran' | 'failed';
  builtMs: number | null;
  runMs: number | null;
  ok: boolean;
  reports: Report[];
  text: string;
  /** True after an edit while the graph is paused: the result is for an old graph. */
  stale: boolean;
}

export const NO_RUN: RunState = { phase: 'none', builtMs: null, runMs: null, ok: true, reports: [], text: '', stale: false };

export interface NodeData extends Record<string, unknown> {
  entry: NodeEntry;
  info: NodeInfo | null;
  wired: string[];
  state: NodeState;
  message: string;
  editable: boolean;
}

export type PatinaNode = Node<NodeData, 'patina'>;

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
  unresolved: 'Unresolved',
};

export function nodeState(entry: NodeEntry, info: NodeInfo | null, view: View, run: RunState): [NodeState, string] {
  if (!info) return ['unresolved', `The folder ${entry.folder ?? entry.subgraph ?? ''} is missing.`];
  if (info.error) return ['compile-error', info.error];
  const problems = nodeProblems(view.problems, entry.id);
  if (problems.length > 0) return ['compile-error', problems.join('\n')];
  if (run.phase === 'building') return ['compiling', ''];
  if (run.phase === 'failed') return ['stale', ''];
  if (run.phase !== 'ran') return ['none', ''];
  const reports = run.reports.filter((r) => r.id === entry.id);
  if (reports.length > 0) {
    return ['run-error', reports.map((r) => (r.kind === 'Panic' ? `Panic: ${r.text}` : r.text)).join('\n')];
  }
  // shortcut: a node after a failed node shows its old state; show "skipped" when the engine reports it.
  return [run.stale ? 'stale' : 'ready', ''];
}

/** The node info of an entry, or null when its folder is missing. */
export function infoOf(entry: NodeEntry, nodes: NodeInfo[]): NodeInfo | null {
  if (!entry.folder?.startsWith('nodes/')) return null;
  const name = folderName(entry);
  return nodes.find((n) => n.name === name) ?? null;
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
  const { entry, info, wired, state, message, editable } = data;
  const inputs = info?.inputs ?? [];
  const outputs = info?.outputs ?? [];
  const rows = Math.max(inputs.length, outputs.length);
  const pinTop = (i: number) => HEADER + PAD + i * ROW + ROW / 2;
  const classes = ['patina-node', state];
  if (selected) classes.push('selected');
  return (
    <div className={classes.join(' ')}>
      <div className="patina-node-header">
        <span className="title">{folderName(entry)}</span>
        <StateIcon state={state} />
      </div>
      <div className="patina-rows" style={{ height: rows * ROW + 2 * PAD }}>
        {Array.from({ length: rows }, (_, i) => {
          const input = inputs[i];
          const text = input && !wired.includes(input) ? entry.params?.[input] : undefined;
          return (
            <div key={i} className="patina-row">
              <span>
                {input}
                {text !== undefined && <span className="param-text"> {text}</span>}
              </span>
              <span>{outputs[i]}</span>
            </div>
          );
        })}
      </div>
      {message && <div className={state === 'unresolved' ? 'patina-node-message muted' : 'patina-node-message'}>{message}</div>}
      {inputs.map((key, i) => (
        <Handle key={`i-${key}`} id={key} type="target" position={Position.Left} isConnectable={editable} style={{ top: pinTop(i) }} />
      ))}
      {outputs.map((key, i) => (
        <Handle key={`o-${key}`} id={key} type="source" position={Position.Right} isConnectable={editable} style={{ top: pinTop(i) }} />
      ))}
    </div>
  );
}

export const NodeView = memo(NodeViewImpl);

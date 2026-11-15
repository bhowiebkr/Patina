// The commands and events of the Rust side (crates/patina-editor/src/session.rs and engine.rs).

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

export type Point = [number, number];

export interface NodeEntry {
  id: string;
  folder?: string;
  subgraph?: string;
  pos?: Point;
  params?: Record<string, string>;
}

export interface Wire {
  from: string;
  to: string;
  points?: Point[];
}

/** The main graph, as in `graphs/main.toml` (spec 6.4). Empty lists are left out. */
export interface GraphFile {
  format: number;
  node?: NodeEntry[];
  wire?: Wire[];
}

export interface NodeInfo {
  name: string;
  inputs: string[];
  outputs: string[];
  error: string | null;
}

export interface View {
  serial: number;
  /** The project ID, which names the saved layout (15.2). */
  id: string;
  root: string;
  name: string;
  graph: GraphFile;
  nodes: NodeInfo[];
  problems: string[];
  dirty: boolean;
  trusted: boolean;
  live: boolean;
  canUndo: boolean;
  canRedo: boolean;
  /** Changes when an edit, undo or redo writes a code file. */
  codeRevision: number;
  /** True when the project has a recovery file and the user did not answer yet (15.10). */
  recovery: boolean;
}

/** The unsaved text of the Code tab, by path. */
export type Code = Record<string, string>;

export interface Recent {
  path: string;
  name: string;
  exists: boolean;
}

export interface StartInfo {
  recent: Recent[];
  projectsDir: string;
  check: string | null;
}

export interface Report {
  id: string;
  folder: string;
  kind: 'Error' | 'Panic' | 'BadParam';
  text: string;
}

/** The live value of an output after a run (spec 15.9), with `from` as "<node id>.<output key>". */
export interface Value {
  from: string;
  text: string;
}

/** A compile error (spec 15.5). Lines and columns start at 1. */
export interface Diagnostic {
  message: string;
  label: string | null;
  rendered: string;
  /** The file from the project root, for example `nodes/number/src/lib.rs`. */
  file: string | null;
  start: [number, number];
  end: [number, number];
}

/** When `errors` of a run is not empty, the build failed and the last build that compiled ran. */
export type EngineEvent =
  | { state: 'building'; serial: number }
  | { state: 'started'; serial: number }
  | { state: 'ran'; serial: number; builtMs: number | null; runMs: number; ok: boolean; stopped: boolean; reports: Report[]; values: Value[]; errors: Diagnostic[] }
  | { state: 'failed'; serial: number; text: string; errors: Diagnostic[] };

export interface OutputText {
  text: string;
  err: boolean;
}

/** One action of the user on the graph (spec 7.4). A pin is `<node ID>.<pin key>`. */
export type Edit =
  | { kind: 'AddNode'; id: string; folder: string; pos: Point }
  | { kind: 'RemoveNode'; id: string }
  | { kind: 'MoveNode'; id: string; pos: Point }
  | { kind: 'Connect'; from: string; to: string }
  | { kind: 'Disconnect'; from: string; to: string }
  | { kind: 'SetRoute'; to: string; points: Point[] }
  | { kind: 'SetParam'; id: string; key: string; text: string | null }
  | { kind: 'SetCode'; path: string; text: string }
  | { kind: 'Group'; edits: Edit[] };

export const api = {
  startInfo: () => invoke<StartInfo>('start_info'),
  newProject: (parent: string, name: string) => invoke<View>('new_project', { parent, name }),
  openProject: (path: string) => invoke<View>('open_project', { path }),
  closeProject: () => invoke<void>('close_project'),
  trustProject: () => invoke<View>('trust_project'),
  edit: (edit: Edit) => invoke<View>('edit', { edit }),
  undo: () => invoke<View>('undo'),
  redo: () => invoke<View>('redo'),
  save: () => invoke<View>('save'),
  nodeFiles: (name: string) => invoke<string[]>('node_files', { name }),
  readCode: (path: string) => invoke<string>('read_code', { path }),
  setUnsavedCode: (code: Code) => invoke<void>('set_unsaved_code', { code }),
  /** Restores or discards the changes of the recovery file (15.10). */
  recover: (restore: boolean) => invoke<{ view: View; code: Code }>('recover', { restore }),
  run: () => invoke<View>('run'),
  pause: () => invoke<View>('pause'),
  /** Pauses, and asks the active run to stop (15.2). */
  stop: () => invoke<View>('stop'),
  /** The ID of the node that runs, or that ran last in the active run. Null between runs (15.2). */
  running: () => invoke<string | null>('running'),
  /** The full text of the live value of an output after the last run (15.7), or null. */
  value: (from: string) => invoke<string | null>('value', { from }),
  /** The saved layout of a project (15.2), or null. */
  layout: (id: string) => invoke<string | null>('layout', { id }),
  saveLayout: (id: string, text: string) => invoke<void>('save_layout', { id, text }),
  quit: () => invoke<void>('quit'),
  checkDone: (result: object) => invoke<void>('check_done', { result }),
  capture: (name: string) => invoke<void>('capture', { name }),
};

export function onEngine(handler: (event: EngineEvent) => void): Promise<UnlistenFn> {
  return listen<EngineEvent>('engine', (e) => handler(e.payload));
}

export function onOutput(handler: (output: OutputText) => void): Promise<UnlistenFn> {
  return listen<OutputText>('output', (e) => handler(e.payload));
}

/** The lines of an output chunk. A chunk holds whole lines, except when a line is very long. */
export function outputLines(text: string): string[] {
  return text.replace(/\r?\n$/, '').split(/\r?\n/);
}

export function onCloseRequested(handler: (value: null) => void): Promise<UnlistenFn> {
  return listen('close-requested', () => handler(null));
}

export function reportError(e: unknown) {
  const message = e instanceof Error ? `${e.message}\n${e.stack ?? ''}` : String(e);
  console.error(message);
  invoke('page_error', { message }).catch(() => {});
}

/** A stable random node ID: 16 lowercase hexadecimal characters (spec 6.4). */
export function newId(): string {
  const bytes = crypto.getRandomValues(new Uint8Array(8));
  return Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
}

/** The folder name of a node entry, for example `number` for `nodes/number`. */
export function folderName(node: NodeEntry): string {
  return (node.folder ?? node.subgraph ?? '?').split('/').pop() ?? '?';
}

/** One line for a compile error, for example `mismatched types: expected f32, found &str`. */
export function errorText(e: Diagnostic): string {
  return e.label ? `${e.message}: ${e.label}` : e.message;
}

/** The problems of the graph check that name the node: `node <id>: <text>`. */
export function nodeProblems(problems: string[], id: string): string[] {
  const prefix = `node ${id}: `;
  return problems.filter((p) => p.startsWith(prefix)).map((p) => p.slice(prefix.length));
}

export const seconds = (ms: number) => `${(ms / 1000).toFixed(1)} s`;
export const millis = (ms: number) => `${ms < 10 ? ms.toFixed(2) : ms.toFixed(0)} ms`;

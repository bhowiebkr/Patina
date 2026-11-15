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
}

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

export type EngineEvent =
  | { state: 'building'; serial: number }
  | { state: 'ran'; serial: number; builtMs: number | null; runMs: number; ok: boolean; reports: Report[] }
  | { state: 'failed'; serial: number; text: string };

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
  | { kind: 'SetParam'; id: string; key: string; text: string | null }
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
  nodeSource: (name: string) => invoke<string>('node_source', { name }),
  run: () => invoke<View>('run'),
  pause: () => invoke<View>('pause'),
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

/** The problems of the graph check that name the node: `node <id>: <text>`. */
export function nodeProblems(problems: string[], id: string): string[] {
  const prefix = `node ${id}: `;
  return problems.filter((p) => p.startsWith(prefix)).map((p) => p.slice(prefix.length));
}

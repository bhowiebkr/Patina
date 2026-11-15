// The commands and events of the Rust side (crates/patina-editor/src/session.rs and engine.rs).

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

export type Point = [number, number];

export interface NodeEntry {
  id: string;
  folder?: string;
  subgraph?: string;
  /** The name of a built-in node, for example `delay` (9.2). */
  builtin?: string;
  pos?: Point;
  params?: Record<string, string>;
}

/** The path of a wire and its corners (spec 15.9). No value uses the graph default. */
export type Style = 'curved' | 'orthogonal' | 'polyline';
export type Corners = 'rounded' | 'sharp';

export interface Wire {
  from: string;
  to: string;
  style?: Style;
  corners?: Corners;
  points?: Point[];
}

/** A public input of a subgraph and the inner inputs that it goes to (spec 6.4). */
export interface PublicInput {
  key: string;
  name: string;
  pos?: Point;
  to?: string[];
}

/** A public output of a subgraph and the inner output that gives its value (spec 6.4). */
export interface PublicOutput {
  key: string;
  name: string;
  pos?: Point;
  from: string;
}

/** A graph file in `graphs/` (spec 6.4). Empty lists are left out. */
export interface GraphFile {
  format: number;
  node?: NodeEntry[];
  wire?: Wire[];
  input?: PublicInput[];
  output?: PublicOutput[];
}

/** An input or an output of a node, with its Rust type (spec 4.1). */
export interface Pin {
  key: string;
  ty: string;
  /** The name of a public pin of a subgraph, whose key is random (6.4). */
  name?: string;
}

export interface NodeInfo {
  name: string;
  inputs: Pin[];
  outputs: Pin[];
  error: string | null;
  /** True for a built-in node, for example the delay (9.2). */
  builtin: boolean;
  /** The type parameters (4.5). A pin of such a type joins a wire of any type. */
  generics: string[];
  /** True when the node GUI has a full size, from `#[node(full_gui)]` (4.3). */
  full: boolean;
  /** True for a subgraph in `graphs/` (6.1). Its name is the file name. */
  subgraph: boolean;
}

export interface View {
  serial: number;
  /** The project ID, which names the saved layout (15.2). */
  id: string;
  root: string;
  name: string;
  graph: GraphFile;
  /** The name of the open graph: `main` or a subgraph (6.1). */
  graphName: string;
  nodes: NodeInfo[];
  /** The widget folders (6.2), for example `gradient-bar` for `widgets/gradient-bar`. */
  widgets: string[];
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

/** The settings screen (15.10). `rust` and `cargo` are null when Patina finds no toolchain. */
export interface Settings {
  rust: string | null;
  cargo: string | null;
  toolchainProblem: string | null;
  /** The toolchain folder (13.3). `ownToolchain` is false when Patina uses the Rust of the system. */
  toolchainDir: string;
  ownToolchain: boolean;
  /** True while the **Install toolchain** button runs. */
  installing: boolean;
  dataDir: string;
  projectsDir: string;
  portable: boolean;
  trusted: string[];
}

export interface Report {
  id: string;
  folder: string;
  kind: 'Error' | 'Panic' | 'BadParam';
  text: string;
}

/** The lanes of a value from its connection kind (spec 6.3): one '0' or '1' for each lane, the most significant first. */
export interface Lanes {
  bits: string;
  label: string;
}

/** The label of a bus (15.9), for example "data[7:0] / 8". */
export function laneLabel({ bits, label }: Lanes): string {
  return `${label}[${bits.length - 1}:0] / ${bits.length}`;
}

/** The live value of an output after a run (spec 15.9), with `from` as "<node id>.<output key>". */
export interface Value {
  from: string;
  text: string;
  lanes?: Lanes;
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

/** The run time of one node call (15.7). */
export interface NodeTime {
  id: string;
  ms: number;
}

/** A log line of a node, from the `log` crate (15.7). */
export interface NodeLog {
  id: string;
  folder: string;
  /** "ERROR", "WARN" or "INFO". */
  level: string;
  text: string;
}

/** The line style of a log line. */
export function logKind(level: string): 'err' | 'warn' | 'out' {
  return level === 'ERROR' ? 'err' : level === 'WARN' ? 'warn' : 'out';
}

/** When `errors` of a run is not empty, the build failed and the last build that compiled ran. */
export type EngineEvent =
  | { state: 'building'; serial: number }
  | { state: 'started'; serial: number }
  | { state: 'ran'; serial: number; builtMs: number | null; slotOnly: boolean; fused: boolean; slot: string | null; runMs: number; ok: boolean; frames: boolean; skipped: string[]; stopped: string[]; times: NodeTime[]; logs: NodeLog[]; reports: Report[]; values: Value[]; errors: Diagnostic[] }
  | { state: 'failed'; serial: number; text: string; errors: Diagnostic[] }
  /** One frame of the frame domain (9.1). Only the nodes of the frame domain ran. */
  | { state: 'frame'; serial: number; runMs: number; ok: boolean; skipped: string[]; stopped: string[]; times: NodeTime[]; logs: NodeLog[]; reports: Report[]; values: Value[] }
  /** The frame run ended after `frames` frames. */
  | { state: 'ended'; serial: number; frames: number }
  /** The widget lines of node GUIs (4.3), and the problems of their `ui` calls. */
  | { state: 'gui'; serial: number; trees: Tree[]; reports: Report[] };

/** The widget lines of a node GUI: mode 0 for the node body, 1 for the Inspector (4.3). */
export interface Tree {
  id: string;
  mode: number;
  tree: string;
}

export interface OutputText {
  text: string;
  err: boolean;
}

/** One action of the user on the graph (spec 7.4). A pin is `<node ID>.<pin key>`. */
export type Edit =
  | { kind: 'AddNode'; id: string; folder: string; builtin?: boolean; pos: Point }
  | { kind: 'RemoveNode'; id: string }
  | { kind: 'MoveNode'; id: string; pos: Point }
  | { kind: 'Connect'; from: string; to: string }
  | { kind: 'Disconnect'; from: string; to: string }
  | { kind: 'SetRoute'; to: string; points: Point[] }
  | { kind: 'SetLook'; to: string; style?: Style; corners?: Corners }
  | { kind: 'SetParam'; id: string; key: string; text: string | null }
  | { kind: 'SetCode'; path: string; text: string }
  | { kind: 'Group'; edits: Edit[] };

export const api = {
  startInfo: () => invoke<StartInfo>('start_info'),
  pickFolder: (start: string) => invoke<string | null>('pick_folder', { start }),
  newProject: (parent: string, name: string) => invoke<View>('new_project', { parent, name }),
  openProject: (path: string) => invoke<View>('open_project', { path }),
  closeProject: () => invoke<void>('close_project'),
  trustProject: () => invoke<View>('trust_project'),
  settings: () => invoke<Settings>('settings'),
  installToolchain: () => invoke<void>('install_toolchain'),
  /** Takes a folder off the list of trusted projects (11), and returns the new list. */
  untrustProject: (path: string) => invoke<string[]>('untrust_project', { path }),
  edit: (edit: Edit) => invoke<View>('edit', { edit }),
  undo: () => invoke<View>('undo'),
  redo: () => invoke<View>('redo'),
  save: () => invoke<View>('save'),
  /** The files of a node or widget folder, for example `nodes/number`. */
  folderFiles: (folder: string) => invoke<string[]>('folder_files', { folder }),
  /** Makes the node folder `nodes/<name>` (15.2). */
  newNode: (name: string) => invoke<View>('new_node', { name }),
  /** Makes the widget folder `widgets/<name>` (4.3). */
  newWidget: (name: string) => invoke<View>('new_widget', { name }),
  /**
   * Adds a crate of crates.io, `name` or `name@version`, or a widget folder of the project to a
   * node or widget folder, for example `nodes/number` (15.4).
   */
  addCrate: (folder: string, spec: string) => invoke<View>('add_crate', { folder, spec }),
  readCode: (path: string) => invoke<string>('read_code', { path }),
  setUnsavedCode: (code: Code) => invoke<void>('set_unsaved_code', { code }),
  /** Restores or discards the changes of the recovery file (15.10). */
  recover: (restore: boolean) => invoke<{ view: View; code: Code }>('recover', { restore }),
  /** Opens the main graph or a subgraph (6.1). The open graph must have no unsaved changes. */
  openGraph: (name: string) => invoke<View>('open_graph', { name }),
  /** Moves the nodes into the new subgraph `name`, with the use `useId` in their place (6.1). */
  collapse: (name: string, ids: string[], useId: string) => invoke<View>('collapse', { name, ids, useId }),
  /** Puts the nodes of each subgraph use in its place (6.1). */
  expand: (ids: string[]) => invoke<View>('expand', { ids }),
  run: () => invoke<View>('run'),
  pause: () => invoke<View>('pause'),
  /** Fuses the node in the slot back into the base now (7.2). */
  bake: () => invoke<View>('bake'),
  /** Pauses, and asks the active run to stop (15.2). */
  stop: () => invoke<View>('stop'),
  /** Sends user input to a node GUI, or asks for its widget lines when `events` is empty (4.3). */
  gui: (id: string, mode: number, events: string) => invoke<void>('gui', { id, mode, events }),
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

/** A step of the toolchain install (13.2). The last event has `done`, and `error` when it failed. */
export interface InstallProgress {
  step: string;
  done: boolean;
  error: string | null;
}

export function onToolchain(handler: (progress: InstallProgress) => void): Promise<UnlistenFn> {
  return listen<InstallProgress>('toolchain', (e) => handler(e.payload));
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
  return (node.folder ?? node.subgraph ?? node.builtin ?? '?').split('/').pop() ?? '?';
}

/** One line for a compile error, for example `mismatched types: expected f32, found &str`. */
export function errorText(e: Diagnostic): string {
  return e.label ? `${e.message}: ${e.label}` : e.message;
}

/** The text of a run report as patina-cli shows it. The text of a panic starts with "panic:". */
export function reportText(r: Report): string {
  return `${r.kind === 'Error' ? 'error: ' : r.kind === 'BadParam' ? 'bad parameter: ' : ''}${r.text}`;
}

/** The problems of the graph check that name the node: `node <id>: <text>`. */
export function nodeProblems(problems: string[], id: string): string[] {
  const prefix = `node ${id}: `;
  return problems.filter((p) => p.startsWith(prefix)).map((p) => p.slice(prefix.length));
}

export const seconds = (ms: number) => `${(ms / 1000).toFixed(1)} s`;
export const millis = (ms: number) => `${ms < 10 ? ms.toFixed(2) : ms.toFixed(0)} ms`;
const micros = (us: number) => `${us < 10 ? us.toFixed(2) : us.toFixed(0)} us`;
/** The run time of a node (15.7): seconds from 1 s, microseconds under 1 ms. */
export const duration = (ms: number) => (ms >= 1000 ? seconds(ms) : ms >= 1 ? millis(ms) : micros(ms * 1000));

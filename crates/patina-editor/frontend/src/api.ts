// The commands and events of the Rust side (crates/patina-editor/src/session.rs and engine.rs).

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { Rect } from './panels/layout';

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

/** The path of a wire and its corners (spec 15.9). A wire with no style uses the graph default. */
export type Style = 'automatic' | 'curved' | 'orthogonal' | 'polyline';
/** A drawn path. No value is the automatic route. */
export type Path = Exclude<Style, 'automatic'>;
export type Corners = 'rounded' | 'sharp';

export interface Wire {
  from: string;
  to: string;
  style?: Style;
  corners?: Corners;
  points?: Point[];
  /** The bus name that replaces the default name (15.9). */
  name?: string;
}

/** An order wire: node `before` runs before node `after`. It carries no data (spec 9.2). */
export interface Order {
  before: string;
  after: string;
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
  /** Not in the file for an output that is not connected yet. */
  from?: string;
}

/** A graph file in `graphs/` (spec 6.4). Empty lists are left out. */
export interface GraphFile {
  format: number;
  /** The path of the wires with no style. No value is the automatic route (spec 15.9). */
  style?: Style;
  node?: NodeEntry[];
  wire?: Wire[];
  order?: Order[];
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

/** The node kind for the node icon (15.3). */
export type NodeKind = 'function' | 'stateful' | 'generic' | 'async' | 'trigger' | 'subgraph';

/** A clock domain (9.1). */
export type Domain = 'batch' | 'frame' | 'audio' | 'event';

/** The clock domains of the nodes of the open graph, with the IDs inside the use `scope` (9.1, 6.4). */
export function scopeDomains(domains: Readonly<Record<string, Domain>>, scope: string): ReadonlyMap<string, Domain> {
  const p = scope ? `${scope}/` : '';
  return new Map(Object.entries(domains).flatMap(([k, d]) => (k.startsWith(p) && !k.includes('/', p.length) ? [[k.slice(p.length), d] as const] : [])));
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
  /** The header color "#rrggbb" of `#[node(color = ...)]`, or null for the gray header (15.3). */
  color: string | null;
  /** True for a subgraph in `graphs/` (6.1). Its name is the file name. */
  subgraph: boolean;
  kind: NodeKind;
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
  /** The view folders (15.8), for example `map` for `views/map`. */
  views: string[];
  /** The connection folders (6.3), for example `byte` for `connections/byte`. */
  connections: string[];
  /** The type folders (5.1), for example `segments` for `types/segments`. */
  types: string[];
  /** The imported folders (6.4), for example `nodes/blur`. */
  imports: string[];
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
  /** The values of the public inputs while the open subgraph runs alone, or null (15.9). */
  test: Record<string, string> | null;
  /** The connection kinds (6.3). */
  kinds: KindInfo[];
  /** The clock domain of each node use of the checked main graph, by flattened ID (9.1). Empty when the check fails. */
  domains: Record<string, Domain>;
  /** The frames each second of the project (9.1). */
  frameRate: number;
  /** The use path of each use of the open subgraph in the main graph, also inside other uses (6.4, 15.9). */
  uses: string[];
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
  /** The project of `patina --shot <project> <dir>`. */
  shot: string | null;
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
  /** The library folders of the node palette (6.2). */
  libraries: Library[];
}

/** The pin of a library copy from Git to a tag and its commit (6.2). */
export interface LibraryPin {
  url: string;
  tag: string;
  commit: string;
  sha256: string;
}

/** A library folder. A copy from Git has its pin. */
export interface Library {
  path: string;
  pin: LibraryPin | null;
}

/** A library of the index (6.2). */
export interface IndexEntry {
  name: string;
  url: string;
  description: string;
  tag: string;
}

/** The result of **Check for updates** for a library from Git (6.2). */
export interface Checked {
  path: string;
  /** The newest tag, when it is newer than the pinned tag. */
  newer?: string | null;
  /** True when the pinned tag points to another commit now. */
  moved?: boolean;
  error: string | null;
}

export interface Report {
  id: string;
  folder: string;
  kind: 'Error' | 'Panic' | 'BadParam';
  text: string;
}

/** The lanes of a value from its connection kind (spec 6.3): one '0' or '1' for each lane, the most significant first. */
/** A connection kind (6.3). `lanes` is empty when the lanes come only with the live values. */
export interface KindInfo {
  ty: string;
  lanes: string[];
  label: string | null;
  color: string | null;
}

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
  /** The thumbnail of the `Viewer` (5.3): "<width> <height> <base64 RGBA>". */
  thumb?: string;
  /** The widget lines of `Viewer::show` (5.3). Only `api.value` gives them. */
  view?: string;
}

/** The size and the RGBA pixels of a thumbnail (5.3), or null for a bad text. */
export function parseThumb(thumb: string) {
  const [w, h, data] = thumb.split(' ');
  const [width, height] = [Number(w), Number(h)];
  const side = (n: number) => Number.isInteger(n) && n >= 1 && n <= 64;
  if (!side(width) || !side(height) || data === undefined) return null;
  try {
    const pixels = Uint8ClampedArray.from(atob(data), (c) => c.charCodeAt(0));
    return pixels.length === width * height * 4 ? { width, height, pixels } : null;
  } catch {
    return null;
  }
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
  /** The start of the call in milliseconds after the start of the run of its domain. */
  start: number;
  /** The worker pool thread of the call. The thread that starts the run is 0. */
  thread: number;
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
  | { state: 'building'; serial: number; base: boolean; slot: string | null }
  | { state: 'started'; serial: number }
  | { state: 'ran'; serial: number; builtMs: number | null; slotOnly: boolean; fused: boolean; slot: string | null; runMs: number; ok: boolean; frames: boolean; events: boolean; skipped: string[]; stopped: string[]; times: NodeTime[]; logs: NodeLog[]; reports: Report[]; values: Value[]; errors: Diagnostic[] }
  | { state: 'failed'; serial: number; text: string; errors: Diagnostic[] }
  /** The clusters of the last build loaded for an open project, and the graph did not run (15.10). */
  | { state: 'loaded'; serial: number; builtMs: number | null; fused: boolean; slot: string | null; errors: Diagnostic[] }
  | { state: 'lost'; serial: number; text: string }
  /** One frame of the frame domain (9.1). Only the nodes of the frame domain ran. */
  | { state: 'frame'; serial: number; frames: number; runMs: number; ok: boolean; skipped: string[]; stopped: string[]; times: NodeTime[]; logs: NodeLog[]; reports: Report[]; values: Value[] }
  /** One event run ended (9.4). Only the nodes of the run ran. `id` is 0 for a report of a trigger or of Patina. */
  | { state: 'fired'; serial: number; id: number; trigger: string; ok: boolean; skipped: string[]; stopped: string[]; times: NodeTime[]; logs: NodeLog[]; reports: Report[]; values: Value[] }
  /** The live run ended after `frames` frames. */
  | { state: 'ended'; serial: number; frames: number }
  /** The widget lines of node GUIs (4.3), the problems of their `ui` calls, and the parameter writes of a GUI that got user input. */
  | { state: 'gui'; serial: number; trees: Tree[]; reports: Report[]; params: ParamWrite[] };

/** A parameter write of a node GUI (4.3), from `Ui::set_param`. */
export interface ParamWrite {
  id: string;
  key: string;
  text: string;
}

/** The edit of the parameter writes of a node GUI (4.3), or null when no write changes the open
 * graph. A later write of the same parameter wins, and a node that is not in the open graph is skipped. */
export function paramEdit(graph: GraphFile, params: readonly ParamWrite[]): Edit | null {
  const last = new Map(params.map((p) => [`${p.id} ${p.key}`, p] as const));
  const edits: Edit[] = [...last.values()]
    .filter((p) => {
      const node = graph.node?.find((n) => n.id === p.id);
      return node !== undefined && node.params?.[p.key] !== p.text;
    })
    .map((p) => ({ kind: 'SetParam', id: p.id, key: p.key, text: p.text }));
  return edits.length === 0 ? null : edits.length === 1 ? edits[0] : { kind: 'Group', edits };
}

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
  | { kind: 'ConnectOrder'; before: string; after: string }
  | { kind: 'DisconnectOrder'; before: string; after: string }
  | { kind: 'SetRoute'; to: string; points: Point[] }
  | { kind: 'SetLook'; to: string; style?: Style; corners?: Corners }
  | { kind: 'SetGraphLook'; style?: Style }
  | { kind: 'NameWire'; to: string; name: string | null }
  | { kind: 'SetParam'; id: string; key: string; text: string | null }
  | { kind: 'SetCode'; path: string; text: string }
  | { kind: 'AddPin'; key: string; name: string; input: boolean; pos: Point }
  | { kind: 'RemovePin'; key: string }
  | { kind: 'MovePin'; key: string; pos: Point }
  | { kind: 'RenamePin'; key: string; name: string }
  | { kind: 'OrderPin'; key: string; index: number }
  | { kind: 'ConnectPin'; key: string; end: string }
  | { kind: 'DisconnectPin'; key: string; end: string }
  | { kind: 'Group'; edits: Edit[] };

/** A folder that an import copies, with its state in this project (6.2). */
export interface ImportItem {
  folder: string;
  state: { kind: 'New' } | { kind: 'Same' } | { kind: 'Conflict'; existing: string; replace: boolean };
  /** The folders of this project that a replace changes. */
  users: string[];
}

/** The differences between an imported folder and its folder in the other project (6.2). */
export interface UpdateChanges {
  /** The other project changed the folder after the import or the last update. */
  other: boolean;
  /** The side that changed each file. An update takes its version, or the pick for `Both`. */
  files: { path: string; state: 'OnlyOther' | 'OnlyThis' | 'Different'; side: 'Other' | 'This' | 'Both' }[];
}

export type UpdatePick = 'Other' | 'This';

export type PathPick = 'open' | 'save' | 'folder';

/** The view after a paste, with the new node IDs and the warnings (15.11). */
export interface Pasted {
  view: View;
  ids: string[];
  warnings: string[];
}

/** A node or a subgraph of a library, for the node palette (6.2). */
export type LibraryNode = NodeInfo & { library: string; path: string };

/** The wire of a pin drag to the pin `key` of a new library node (15.3). `output` is true when `end` is an output. */
export interface DragLink {
  end: string;
  output: boolean;
  key: string;
}

export type ImportChoice = { kind: 'Replace' } | { kind: 'Keep' } | { kind: 'Rename'; name: string };

export const api = {
  startInfo: () => invoke<StartInfo>('start_info'),
  pickFolder: (start: string) => invoke<string | null>('pick_folder', { start }),
  /** The **Browse** dialog of a file path widget (4.3). A path in the project comes back relative. */
  pickPath: (kind: PathPick, start: string) => invoke<string | null>('pick_path', { kind, start }),
  /** The project files with one of the extensions, for an asset picker (4.3). */
  assetFiles: (extensions: string[]) => invoke<string[]>('asset_files', { extensions }),
  newProject: (parent: string, name: string) => invoke<View>('new_project', { parent, name }),
  openProject: (path: string) => invoke<View>('open_project', { path }),
  closeProject: () => invoke<void>('close_project'),
  trustProject: () => invoke<View>('trust_project'),
  settings: () => invoke<Settings>('settings'),
  installToolchain: () => invoke<void>('install_toolchain'),
  updateToolchain: () => invoke<void>('update_toolchain'),
  analyzerStart: () => invoke<number>('analyzer_start'),
  analyzerSend: (serial: number, text: string) => invoke<void>('analyzer_send', { serial, text }),
  analyzerStop: () => invoke<void>('analyzer_stop'),
  /** Reads the newest release from GitHub (15.3). Null means that Patina has no release yet. */
  checkUpdate: () => invoke<Release | null>('check_update'),
  /** Takes a folder off the list of trusted projects (11), and returns the new list. */
  untrustProject: (path: string) => invoke<string[]>('untrust_project', { path }),
  /** The clip text of the nodes, for the clipboard (15.11). */
  copyClip: (ids: string[]) => invoke<string>('copy_clip', { ids }),
  pasteClip: (text: string, at: [number, number]) => invoke<Pasted>('paste_clip', { text, at }),
  /** Adds a node or a subgraph of a library, as a paste of one node (6.2). */
  pasteLibrary: (n: LibraryNode, at: [number, number], link?: DragLink) => invoke<Pasted>('paste_library', { path: n.path, name: n.name, subgraph: n.subgraph, at, link: link ?? null }),
  libraryNodes: () => invoke<LibraryNode[]>('library_nodes'),
  /** Adds a Patina project to the libraries and trusts it (6.2), and returns the new list. */
  addLibrary: (path: string) => invoke<Library[]>('add_library', { path }),
  /** Takes a folder off the libraries. A copy from Git is deleted (6.2). */
  removeLibrary: (path: string) => invoke<Library[]>('remove_library', { path }),
  /** Downloads a tag of a Git repository, or the newest tag when `tag` is empty, as a library (6.2). */
  addGitLibrary: (url: string, tag: string) => invoke<Library[]>('add_git_library', { url, tag }),
  checkLibraries: () => invoke<Checked[]>('check_libraries'),
  /** Downloads the index of the shared libraries (6.2). */
  libraryIndex: () => invoke<IndexEntry[]>('library_index'),
  /** Replaces a copy from Git with another tag of its repository (6.2). */
  updateLibrary: (path: string, tag: string) => invoke<Library[]>('update_library', { path, tag }),
  edit: (edit: Edit) => invoke<View>('edit', { edit }),
  undo: () => invoke<View>('undo'),
  redo: () => invoke<View>('redo'),
  save: () => invoke<View>('save'),
  /** The files of a code folder, for example `nodes/number`. */
  folderFiles: (folder: string) => invoke<string[]>('folder_files', { folder }),
  /** Makes the node folder `nodes/<name>` (15.2). */
  newNode: (name: string) => invoke<View>('new_node', { name }),
  /** Makes the widget folder `widgets/<name>` (4.3). */
  newWidget: (name: string) => invoke<View>('new_widget', { name }),
  /** Makes the view folder `views/<name>` (15.8). */
  newView: (name: string) => invoke<View>('new_view', { name }),
  /** Makes the connection folder `connections/<name>` (6.3). */
  newConnection: (name: string) => invoke<View>('new_connection', { name }),
  /** Makes the type folder `types/<name>` (5.1). */
  newType: (name: string) => invoke<View>('new_type', { name }),
  /**
   * Gives the user input `events` to view `name` and gets its widget lines (15.8). The first call
   * builds the view. `view` is the new project when the view wrote to the graph.
   */
  viewUi: (name: string, instance: string, events: string, selected: string[]) =>
    invoke<{ tree: string; view: View | null; select: string[] | null }>('view_ui', { name, instance, events, selected }),
  /** Drops the view instance of a dock slot. */
  closeView: (instance: string) => invoke<void>('close_view', { instance }),
  /** Opens a window for each float view slot and closes the other float windows (15.8). */
  setFloats: (floats: { slot: string; name: string; title: string; rect: Rect | null }[]) => invoke<void>('set_floats', { floats }),
  /** The native handle of the editor window for a viewport, or 0 where a viewport cannot draw. */
  windowHandle: () => invoke<number>('window_handle'),
  /**
   * Adds a crate of crates.io, `name` or `name@version`, or a widget or type folder of the
   * project to a code folder, for example `nodes/number` (15.4).
   */
  addCrate: (folder: string, spec: string) => invoke<View>('add_crate', { folder, spec }),
  /** The folders of the trusted project at `other` that Import can copy (6.2). */
  importFolders: (other: string) => invoke<string[]>('import_folders', { other }),
  importPlan: (other: string, folder: string) => invoke<ImportItem[]>('import_plan', { other, folder }),
  /** Copies the folders with a choice for each conflict, and builds them first. */
  importFolder: (other: string, folder: string, choices: Record<string, ImportChoice>) =>
    invoke<View>('import_folder', { other, folder, choices }),
  /** The trusted project that an imported folder came from, or null when Patina does not know it. */
  updateSource: (folder: string) => invoke<string | null>('update_source', { folder }),
  /** Makes a package of the saved project in `<parent>/<project name>` (14.1). */
  makePackage: (parent: string) => invoke<{ out: string; warnings: string[] }>('make_package', { parent }),
  updateChanges: (other: string, folder: string) => invoke<UpdateChanges>('update_changes', { other, folder }),
  /** Writes the new version of an imported folder, and builds it first. */
  updateFolder: (other: string, folder: string, picks: Record<string, UpdatePick>) =>
    invoke<View>('update_folder', { other, folder, picks }),
  readCode: (path: string) => invoke<string>('read_code', { path }),
  setUnsavedCode: (code: Code) => invoke<void>('set_unsaved_code', { code }),
  /** Restores or discards the changes of the recovery file (15.10). */
  recover: (restore: boolean) => invoke<{ view: View; code: Code }>('recover', { restore }),
  /** Opens the main graph or a subgraph (6.1). The open graph must have no unsaved changes. */
  openGraph: (name: string) => invoke<View>('open_graph', { name }),
  /** The file of a subgraph, with its unsaved edits, to show its nodes in place (15.3). */
  subgraphFile: (name: string) => invoke<GraphFile>('subgraph_file', { name }),
  /** Moves the nodes into the new subgraph `name`, with the use `useId` in their place (6.1). */
  collapse: (name: string, ids: string[], useId: string) => invoke<View>('collapse', { name, ids, useId }),
  /** Puts the nodes of each subgraph use in its place (6.1). */
  expand: (ids: string[]) => invoke<View>('expand', { ids }),
  run: () => invoke<View>('run'),
  pause: () => invoke<View>('pause'),
  step: () => invoke<View>('step'),
  /** Sets the frames each second of the project, 1 to 100000 (9.1). */
  frameRate: (rate: number) => invoke<View>('frame_rate', { rate }),
  /** Runs the open subgraph alone with these public input values. Null ends the test (15.9). */
  test: (inputs: Record<string, string> | null) => invoke<View>('test', { inputs }),
  /** Fuses the node in the slot back into the base now (7.2). */
  bake: () => invoke<View>('bake'),
  /** Pauses, and asks the active run to stop (15.2). */
  stop: () => invoke<View>('stop'),
  /** Sends user input to a node GUI, or asks for its widget lines when `events` is empty (4.3). */
  gui: (id: string, mode: number, events: string) => invoke<void>('gui', { id, mode, events }),
  /** Gives the nodes in view. The frames draw only their GUIs (4.3 rule 3). */
  visible: (ids: string[]) => invoke<void>('visible', { ids }),
  /** The ID of the node that runs, or that ran last in the active run. Null between runs (15.2). */
  running: () => invoke<string | null>('running'),
  /** Fires the `trigger` node `id` of the live run with `text` (9.4). */
  fire: (id: string, text: string) => invoke<void>('fire', { id, text }),
  /** The live value of an output after the last run, with its full text and widgets (15.7), or null. */
  value: (from: string) => invoke<Value | null>('value', { from }),
  /** The saved layout of a project (15.2), or null. */
  layout: (id: string) => invoke<string | null>('layout', { id }),
  saveLayout: (id: string, text: string) => invoke<void>('save_layout', { id, text }),
  layoutPending: (pending: boolean) => invoke<void>('layout_pending', { pending }),
  quit: () => invoke<void>('quit'),
  checkDone: (result: object) => invoke<void>('check_done', { result }),
  capture: (name: string) => invoke<void>('capture', { name }),
};

export function onEngine(handler: (event: EngineEvent) => void): Promise<UnlistenFn> {
  return listen<EngineEvent>('engine', (e) => handler(e.payload));
}

/** The newest release of Patina on GitHub (15.3). `newer` is true when it is newer than this editor. */
export interface Release {
  version: string;
  url: string;
  newer: boolean;
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

/** A message of rust-analyzer start `serial`, or null text when the process stopped (15.1). */
export function onAnalyzer(handler: (message: { serial: number; text: string | null }) => void): Promise<UnlistenFn> {
  return listen<{ serial: number; text: string | null }>('analyzer', (e) => handler(e.payload));
}

export function onOutput(handler: (output: OutputText) => void): Promise<UnlistenFn> {
  return listen<OutputText>('output', (e) => handler(e.payload));
}

/** The lines of an output chunk. A chunk holds whole lines, except when a line is very long. */
export function outputLines(text: string): string[] {
  return text.replace(/\r?\n$/, '').split(/\r?\n/);
}

/** The window wants to close. `ask` is true when the project has unsaved changes. */
export function onCloseRequested(handler: (ask: boolean) => void): Promise<UnlistenFn> {
  return listen<boolean>('close-requested', (e) => handler(e.payload));
}

export function reportError(e: unknown) {
  const message = e instanceof Error ? `${e.message}\n${e.stack ?? ''}` : String(e);
  console.error(message);
  invoke('page_error', { message }).catch(() => {});
}

/** A stable random node ID: 16 lowercase hexadecimal characters (spec 6.4). */
export function newId(size = 8): string {
  const bytes = crypto.getRandomValues(new Uint8Array(size));
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

/** True for the problem of a generic node that has no type for a parameter (4.5, 9.2). The node is unresolved (15.2). */
export const openType = (problem: string) => /^connect a wire (that gives the type of |to the input `value` of the delay)/.test(problem);

export const seconds = (ms: number) => `${(ms / 1000).toFixed(1)} s`;
export const millis = (ms: number) => `${ms < 10 ? ms.toFixed(2) : ms.toFixed(0)} ms`;
/** The first sentence of an error text, for the status bar. */
export const firstSentence = (text: string) => text.split('\n')[0].replace(/\. .*$/, '.');
const micros = (us: number) => `${us < 10 ? us.toFixed(2) : us.toFixed(0)} us`;
/** The run time of a node (15.7): seconds from 1 s, microseconds under 1 ms. */
export const duration = (ms: number) => (ms >= 1000 ? seconds(ms) : ms >= 1 ? millis(ms) : micros(ms * 1000));

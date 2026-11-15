// The code buffers of the Code tab (spec 15.10): one Monaco model for each open file of a node
// folder. Edits stay in the buffer until a save sends one SetCode edit for each changed file.

// shortcut: no rust-analyzer; connect it at milestone 3 (spec 15.1).
// shortcut: Monaco is in the main script (3.7 MB); load it with the Code tab if the start gets slow.
import * as monaco from 'monaco-editor/editor/editor.api';
import 'monaco-editor/editor/contrib/bracketMatching/browser/bracketMatching';
import 'monaco-editor/editor/contrib/clipboard/browser/clipboard';
import 'monaco-editor/editor/contrib/comment/browser/comment';
import 'monaco-editor/editor/contrib/cursorUndo/browser/cursorUndo';
import 'monaco-editor/editor/contrib/folding/browser/folding';
import 'monaco-editor/editor/contrib/hover/browser/hoverContribution';
import 'monaco-editor/editor/contrib/indentation/browser/indentation';
import 'monaco-editor/editor/contrib/linesOperations/browser/linesOperations';
import 'monaco-editor/editor/contrib/multicursor/browser/multicursor';
import 'monaco-editor/editor/contrib/wordHighlighter/browser/wordHighlighter';
import 'monaco-editor/editor/contrib/wordOperations/browser/wordOperations';
import 'monaco-editor/features/find/register';
import 'monaco-editor/languages/definitions/rust/register';
import EditorWorker from 'monaco-editor/editor/editor.worker?worker';
import { api, errorText, type Code, type Diagnostic, type View } from './api';

export { monaco };

self.MonacoEnvironment = { getWorker: () => new EditorWorker() };

// The colors of theme.css (docs/gui-visual-guide.md).
monaco.editor.defineTheme('patina', {
  base: 'vs-dark',
  inherit: true,
  rules: [],
  colors: {
    'editor.background': '#191919',
    'editor.foreground': '#eeeeeb',
    'editorGutter.background': '#191919',
    'editorLineNumber.foreground': '#73736e',
    'editorLineNumber.activeForeground': '#b2b2ad',
    'editor.lineHighlightBackground': '#202020',
    'editorCursor.foreground': '#dfa64b',
    'editor.selectionBackground': '#dfa64b40',
    'editorError.foreground': '#ed7b78',
    focusBorder: '#dfa64b',
  },
});

interface Buffer {
  model: monaco.editor.ITextModel;
  /** The text of the file on disk, as the last read or save found it. */
  saved: string;
  /** The ids of the gutter marks of the compile errors. */
  marks: string[];
}

const buffers = new Map<string, Buffer>();
let errors: Diagnostic[] = [];
let unsaved: string[] = [];
const listeners = new Set<() => void>();
let sendTimer: ReturnType<typeof setTimeout> | undefined;
let reportError: (e: string) => void = () => {};

function changed(buffer: Buffer): boolean {
  return buffer.model.getValue() !== buffer.saved;
}

/** Sets the function that shows an error of the code buffers, for example of the recovery file. */
export function onCodeError(f: (e: string) => void) {
  reportError = f;
}

/** Sends the unsaved text to the Rust side now, for the close question and the recovery file (15.10). */
export function flushCode(): Promise<void> {
  clearTimeout(sendTimer);
  const code = Object.fromEntries(unsaved.map((path) => [path, buffers.get(path)!.model.getValue()]));
  return api.setUnsavedCode(code).catch((e) => reportError(String(e)));
}

/** Finds the changed files again and tells the page when the list changes. */
function update() {
  const now = [...buffers].filter(([, b]) => changed(b)).map(([path]) => path);
  const had = unsaved.length > 0;
  if (now.join('\n') !== unsaved.join('\n')) {
    unsaved = now;
    listeners.forEach((f) => f());
  }
  // The close question must know at once that there are unsaved changes. Typing waits for a pause.
  if ((now.length > 0) !== had) flushCode();
  else if (had) {
    clearTimeout(sendTimer);
    sendTimer = setTimeout(flushCode, 300);
  }
}

export function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** The paths of the files with unsaved changes. The list stays the same object until it changes. */
export function unsavedFiles(): string[] {
  return unsaved;
}

function mark(path: string, buffer: Buffer) {
  const mine = errors.filter((e) => e.file === path);
  const range = (e: Diagnostic) => ({
    startLineNumber: e.start[0],
    startColumn: e.start[1],
    endLineNumber: e.end[0],
    endColumn: e.end[1],
  });
  const markers = mine.map((e) => ({ ...range(e), severity: monaco.MarkerSeverity.Error, message: errorText(e), source: 'rustc' }));
  monaco.editor.setModelMarkers(buffer.model, 'rustc', markers);
  const gutter = mine.map((e) => ({ range: range(e), options: { glyphMarginClassName: 'gutter-error', glyphMarginHoverMessage: { value: errorText(e) } } }));
  buffer.marks = buffer.model.deltaDecorations(buffer.marks, gutter);
}

/** Shows the compile errors of the last build in each open file (15.5). */
export function showErrors(list: Diagnostic[]) {
  errors = list;
  buffers.forEach((buffer, path) => mark(path, buffer));
}

/** The buffer of a file. The first call reads the file. */
export async function openFile(path: string): Promise<monaco.editor.ITextModel> {
  const old = buffers.get(path);
  if (old) return old.model;
  const text = await api.readCode(path);
  const again = buffers.get(path);
  if (again) return again.model;
  const model = monaco.editor.createModel(text, undefined, monaco.Uri.from({ scheme: 'patina', path: `/${path}` }));
  const buffer = { model, saved: text, marks: [] };
  buffers.set(path, buffer);
  model.onDidChangeContent(update);
  mark(path, buffer);
  return model;
}

/** The buffer of a file that is open, for the self-check. */
export function bufferOf(path: string): monaco.editor.ITextModel | undefined {
  return buffers.get(path)?.model;
}

/** Sends one SetCode edit for each changed file. Returns the last view, or null with no change. */
export async function saveCode(): Promise<View | null> {
  let view: View | null = null;
  for (const path of unsaved) {
    const buffer = buffers.get(path);
    if (!buffer) continue;
    const text = buffer.model.getValue();
    view = await api.edit({ kind: 'SetCode', path, text });
    buffer.saved = text;
  }
  update();
  return view;
}

/** Reads the open files again after undo or redo wrote one. A buffer with changes keeps them. */
export async function reloadCode() {
  for (const [path, buffer] of buffers) {
    const text = await api.readCode(path).catch(() => null);
    if (text === null) {
      buffer.model.dispose();
      buffers.delete(path);
    } else if (text !== buffer.saved) {
      const clean = !changed(buffer);
      buffer.saved = text;
      if (clean) buffer.model.setValue(text);
    }
  }
  update();
}

/** Puts the text of a recovery file into the buffers (15.10). Returns an error, or ''. */
export async function restoreCode(code: Code): Promise<string> {
  const lost: string[] = [];
  for (const [path, text] of Object.entries(code)) {
    try {
      (await openFile(path)).setValue(text);
    } catch {
      lost.push(path);
    }
  }
  return lost.length === 0 ? '' : `Patina cannot open ${lost.join(', ')}. The unsaved changes of these files are lost.`;
}

/** Closes all buffers when another project opens. */
export function closeCode() {
  buffers.forEach((buffer) => buffer.model.dispose());
  buffers.clear();
  errors = [];
  update();
}

// The client of rust-analyzer for the Code tab (spec 15.1): completion and hover. Rust starts the
// process for a trusted project and passes the messages. The compile errors come from the build.

import * as monaco from 'monaco-editor/editor/editor.api';
import { api, onAnalyzer } from '../api';
import { completionItems, docText, edits, hoverText, type LspCompletion, type LspCompletionItem, type Suggestion, suggestion } from './lspMap';

interface Server {
  serial: number;
  next: number;
  /** The open requests, by ID. Each one gets the result, or null for an error. */
  pending: Map<number, (result: unknown) => void>;
  /** The sends go out in order. */
  chain: Promise<unknown>;
  /** The paths of the files that the server has open. */
  open: Set<string>;
}

const CAPABILITIES = {
  general: { positionEncodings: ['utf-16'] },
  textDocument: {
    synchronization: { didSave: false },
    completion: {
      completionItem: {
        snippetSupport: true,
        documentationFormat: ['markdown', 'plaintext'],
        deprecatedSupport: true,
        tagSupport: { valueSet: [1] },
        insertReplaceSupport: true,
        labelDetailsSupport: true,
        resolveSupport: { properties: ['documentation', 'detail', 'additionalTextEdits'] },
      },
    },
    hover: { contentFormat: ['markdown', 'plaintext'] },
  },
};

let root: string | null = null;
let project = '';
let generation = 0;
let starting: Promise<Server | null> | null = null;
let current: Server | null = null;
let listening: Promise<unknown> | null = null;
/** The open buffers of the Code tab, by path. */
const tracked = new Map<string, monaco.editor.ITextModel>();

// A link in a hover or a document must not load a page in the editor window.
monaco.editor.registerLinkOpener({ open: () => true });

/** Sets the project of rust-analyzer: the root of a trusted project and the serial of its opening, or null. A change stops the old process. */
export function analyzerProject(next: string | null, serial: number) {
  const key = next === null ? '' : `${serial}
${next}`;
  if (key === project) return;
  project = key;
  root = next;
  stop();
}

function stop() {
  generation++;
  if (starting) api.analyzerStop().catch(() => {});
  current?.pending.forEach((resolve) => resolve(null));
  current = null;
  starting = null;
}

/** The server of the project. The first call starts it. A failed start does not try again for this project. */
function server(): Promise<Server | null> {
  if (!starting && root) starting = start(root, generation);
  return starting ?? Promise.resolve(null);
}

function fileUri(path: string): string {
  return monaco.Uri.file(`${root}/${path}`.replaceAll('\\', '/')).toString();
}

async function start(at: string, gen: number): Promise<Server | null> {
  listening ??= onAnalyzer(receive);
  await listening;
  let serial: number;
  try {
    serial = await api.analyzerStart();
  } catch (e) {
    console.warn(`rust-analyzer: ${e}`);
    return null;
  }
  if (gen !== generation) {
    if (!starting) api.analyzerStop().catch(() => {});
    return null;
  }
  const s: Server = { serial, next: 1, pending: new Map(), chain: Promise.resolve(), open: new Set() };
  current = s;
  const uri = monaco.Uri.file(at.replaceAll('\\', '/')).toString();
  const result = await request(s, 'initialize', {
    processId: null,
    rootUri: uri,
    workspaceFolders: [{ uri, name: 'project' }],
    capabilities: CAPABILITIES,
    initializationOptions: { checkOnSave: false },
  });
  if (current !== s || result === null) return null;
  notify(s, 'initialized', {});
  tracked.forEach((model, path) => openIn(s, path, model));
  return s;
}

function receive(message: { serial: number; text: string | null }) {
  const s = current;
  if (!s || message.serial !== s.serial) return;
  if (message.text === null) {
    console.warn('rust-analyzer stopped.');
    s.pending.forEach((resolve) => resolve(null));
    current = null;
    return;
  }
  let m: { id?: number | string; method?: string; result?: unknown; error?: unknown };
  try {
    m = JSON.parse(message.text);
  } catch {
    return;
  }
  // The server asks, for example, to create a progress bar. The client has none, so null answers all.
  if (m.id !== undefined && m.method) send(s, { id: m.id, result: null });
  else if (typeof m.id === 'number') {
    const resolve = s.pending.get(m.id);
    s.pending.delete(m.id);
    resolve?.(m.error ? null : (m.result ?? null));
  }
}

function send(s: Server, message: object): Promise<boolean> {
  const text = JSON.stringify({ jsonrpc: '2.0', ...message });
  const sent = s.chain.then(() => api.analyzerSend(s.serial, text)).then(
    () => true,
    () => false,
  );
  s.chain = sent;
  return sent;
}

function notify(s: Server, method: string, params: unknown) {
  send(s, { method, params });
}

function request(s: Server, method: string, params: unknown, token?: monaco.CancellationToken): Promise<unknown> {
  const id = s.next++;
  return new Promise((resolve) => {
    s.pending.set(id, resolve);
    send(s, { id, method, params }).then((ok) => {
      if (!ok && s.pending.delete(id)) resolve(null);
    });
    token?.onCancellationRequested(() => {
      if (s.pending.delete(id)) {
        resolve(null);
        notify(s, '$/cancelRequest', { id });
      }
    });
  });
}

function openIn(s: Server, path: string, model: monaco.editor.ITextModel) {
  if (s.open.has(path) || model.isDisposed()) return;
  s.open.add(path);
  notify(s, 'textDocument/didOpen', { textDocument: { uri: fileUri(path), languageId: 'rust', version: model.getVersionId(), text: model.getValue() } });
}

/** Gives a buffer of the Code tab to rust-analyzer. Only Rust files go to the server. */
export function trackModel(path: string, model: monaco.editor.ITextModel) {
  if (!path.endsWith('.rs')) return;
  tracked.set(path, model);
  server().then((s) => s && openIn(s, path, model));
  model.onDidChangeContent(() => {
    const s = current;
    if (s?.open.has(path)) notify(s, 'textDocument/didChange', { textDocument: { uri: fileUri(path), version: model.getVersionId() }, contentChanges: [{ text: model.getValue() }] });
  });
  model.onWillDispose(() => {
    tracked.delete(path);
    const s = current;
    if (s?.open.delete(path)) notify(s, 'textDocument/didClose', { textDocument: { uri: fileUri(path) } });
  });
}

/** The path of a buffer of the Code tab, or null for another model. */
function pathOf(model: monaco.editor.ITextModel): string | null {
  const path = model.uri.scheme === 'patina' ? model.uri.path.slice(1) : null;
  return path !== null && tracked.get(path) === model ? path : null;
}

async function ready(model: monaco.editor.ITextModel): Promise<{ s: Server; path: string } | null> {
  const path = pathOf(model);
  const s = path === null ? null : await server();
  return s && path !== null && s.open.has(path) ? { s, path } : null;
}

const position = (p: monaco.Position) => ({ line: p.lineNumber - 1, character: p.column - 1 });

type Item = monaco.languages.CompletionItem & { item: LspCompletionItem };

function toItem(sug: Suggestion): Item {
  return {
    ...sug,
    kind: monaco.languages.CompletionItemKind[sug.kind],
    tags: sug.deprecated ? [monaco.languages.CompletionItemTag.Deprecated] : undefined,
    insertTextRules: sug.snippet ? monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet : undefined,
  };
}

monaco.languages.registerCompletionItemProvider('rust', {
  triggerCharacters: ['.', ':'],
  async provideCompletionItems(model, at, _context, token) {
    const on = await ready(model);
    if (!on) return { suggestions: [] };
    const result = (await request(on.s, 'textDocument/completion', { textDocument: { uri: fileUri(on.path) }, position: position(at) }, token)) as LspCompletion;
    const word = model.getWordUntilPosition(at);
    const range = { startLineNumber: at.lineNumber, startColumn: word.startColumn, endLineNumber: at.lineNumber, endColumn: word.endColumn };
    const { items, incomplete } = completionItems(result);
    return { suggestions: items.map((item) => toItem(suggestion(item, range))), incomplete };
  },
  async resolveCompletionItem(item, token) {
    const s = current;
    const raw = (item as Item).item;
    if (!s || !raw) return item;
    const full = (await request(s, 'completionItem/resolve', raw, token)) as LspCompletionItem | null;
    if (!full) return item;
    return { ...item, detail: full.detail ?? item.detail, documentation: docText(full.documentation) ?? item.documentation, additionalTextEdits: edits(full.additionalTextEdits) ?? item.additionalTextEdits };
  },
});

monaco.languages.registerHoverProvider('rust', {
  async provideHover(model, at, token) {
    const on = await ready(model);
    if (!on) return null;
    const result = (await request(on.s, 'textDocument/hover', { textDocument: { uri: fileUri(on.path) }, position: position(at) }, token)) as { contents?: Parameters<typeof hoverText>[0] } | null;
    const text = hoverText(result?.contents);
    return text === '' ? null : { contents: [{ value: text }] };
  },
});

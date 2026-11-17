// The conversion of language server results to the shapes of Monaco (spec 15.1). These functions do
// not use Monaco, so `npm test` runs them in Node.

export interface LspPosition {
  line: number;
  character: number;
}

export interface LspRange {
  start: LspPosition;
  end: LspPosition;
}

export interface MarkupContent {
  kind: 'plaintext' | 'markdown';
  value: string;
}

type MarkedString = string | { language: string; value: string };

export interface LspEdit {
  range: LspRange;
  newText: string;
}

export interface LspCompletionItem {
  label: string;
  labelDetails?: { detail?: string; description?: string };
  kind?: number;
  tags?: number[];
  detail?: string;
  documentation?: string | MarkupContent;
  deprecated?: boolean;
  preselect?: boolean;
  sortText?: string;
  filterText?: string;
  insertText?: string;
  insertTextFormat?: number;
  textEdit?: LspEdit | { newText: string; insert: LspRange; replace: LspRange };
  additionalTextEdits?: LspEdit[];
  data?: unknown;
}

export type LspCompletion = LspCompletionItem[] | { isIncomplete: boolean; items: LspCompletionItem[] } | null;

export interface Range {
  startLineNumber: number;
  startColumn: number;
  endLineNumber: number;
  endColumn: number;
}

/** The names of the LSP completion kinds, in the order of their numbers from 1. Monaco has the same names. */
const KINDS = [
  'Text', 'Method', 'Function', 'Constructor', 'Field', 'Variable', 'Class', 'Interface', 'Module', 'Property', 'Unit', 'Value', 'Enum',
  'Keyword', 'Snippet', 'Color', 'File', 'Reference', 'Folder', 'EnumMember', 'Constant', 'Struct', 'Event', 'Operator', 'TypeParameter',
] as const;

export type KindName = (typeof KINDS)[number];

export interface Suggestion {
  label: string | { label: string; detail?: string; description?: string };
  kind: KindName;
  detail?: string;
  documentation?: string | { value: string };
  deprecated: boolean;
  preselect?: boolean;
  sortText?: string;
  filterText?: string;
  insertText: string;
  snippet: boolean;
  range: Range | { insert: Range; replace: Range };
  additionalTextEdits?: { range: Range; text: string }[];
  /** The item of the server, for `completionItem/resolve`. */
  item: LspCompletionItem;
}

/** LSP positions count from 0, Monaco positions from 1. Both count UTF-16 units. */
export function toRange(r: LspRange): Range {
  return { startLineNumber: r.start.line + 1, startColumn: r.start.character + 1, endLineNumber: r.end.line + 1, endColumn: r.end.character + 1 };
}

export function kindName(kind: number | undefined): KindName {
  return KINDS[(kind ?? 1) - 1] ?? 'Text';
}

export function docText(doc: string | MarkupContent | undefined): string | { value: string } | undefined {
  if (doc === undefined || typeof doc === 'string') return doc;
  return doc.kind === 'markdown' ? { value: doc.value } : doc.value;
}

export function edits(list: LspEdit[] | undefined): { range: Range; text: string }[] | undefined {
  return list?.map((e) => ({ range: toRange(e.range), text: e.newText }));
}

/** Converts one completion item. `word` is the range of the word at the cursor, for an item without a text edit. */
export function suggestion(item: LspCompletionItem, word: Range): Suggestion {
  const edit = item.textEdit;
  const range = !edit ? word : 'range' in edit ? toRange(edit.range) : { insert: toRange(edit.insert), replace: toRange(edit.replace) };
  return {
    label: item.labelDetails ? { label: item.label, ...item.labelDetails } : item.label,
    kind: kindName(item.kind),
    detail: item.detail,
    documentation: docText(item.documentation),
    deprecated: item.deprecated === true || (item.tags ?? []).includes(1),
    preselect: item.preselect,
    sortText: item.sortText,
    filterText: item.filterText,
    insertText: edit?.newText ?? item.insertText ?? item.label,
    snippet: item.insertTextFormat === 2,
    range,
    additionalTextEdits: edits(item.additionalTextEdits),
    item,
  };
}

/** The items and the `isIncomplete` flag of a completion result. */
export function completionItems(result: LspCompletion): { items: LspCompletionItem[]; incomplete: boolean } {
  if (!result) return { items: [], incomplete: false };
  return Array.isArray(result) ? { items: result, incomplete: false } : { items: result.items, incomplete: result.isIncomplete };
}

/** The Markdown text of the `contents` of a hover result, or '' when it has none. */
export function hoverText(contents: MarkupContent | MarkedString | MarkedString[] | null | undefined): string {
  if (!contents) return '';
  const one = (c: MarkupContent | MarkedString) => (typeof c === 'string' ? c : 'kind' in c ? c.value : `\`\`\`${c.language}\n${c.value}\n\`\`\``);
  return (Array.isArray(contents) ? contents : [contents]).map(one).filter((t) => t !== '').join('\n\n---\n\n');
}

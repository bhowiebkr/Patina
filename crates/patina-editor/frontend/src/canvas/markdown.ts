// A safe Markdown subset for the notes on the canvas. A note can come from an untrusted library,
// so the parser makes plain data only, runs in linear time and limits the nesting.

/** The longest note text that the parser reads. It cuts a longer text. */
export const MAX_CHARS = 20000;
/** The deepest nesting of lists and quotes. A deeper marker stays plain text. */
export const MAX_DEPTH = 4;
/** The deepest nesting of inline marks and links. A deeper mark has no effect. */
export const MAX_INLINE = 8;
// A link target scan stops after this number of characters, so the time stays linear.
const MAX_TARGET = 2048;

export type Align = 'left' | 'center' | 'right' | null;

export type Mark = 'em' | 'strong' | 'del';

export type Inline =
  | { type: 'text'; text: string }
  | { type: 'code'; text: string }
  | { type: 'br' }
  | { type: Mark; children: Inline[] }
  | { type: 'link'; url: string; children: Inline[] }
  | { type: 'image'; src: string; alt: string };

export interface ListItem {
  /** True or false for a task item, null for a usual item. */
  checked: boolean | null;
  blocks: Block[];
}

export type Block =
  | { type: 'heading'; level: number; content: Inline[] }
  | { type: 'paragraph'; content: Inline[] }
  | { type: 'list'; ordered: boolean; start: number; items: ListItem[] }
  | { type: 'table'; align: Align[]; head: Inline[][]; rows: Inline[][][] }
  | { type: 'code'; lang: string; text: string }
  | { type: 'quote'; blocks: Block[] }
  | { type: 'rule' };

/** The blocks of a note text. Raw HTML stays text, and the result never holds markup. */
export function parseMarkdown(text: string): Block[] {
  const lines = text.slice(0, MAX_CHARS).replace(/\r\n?/g, '\n').split('\n').map(detab);
  return blocks(lines, 0);
}

const HEADING = /^ {0,3}(#{1,6})(?:[ \t]|$)/;
const QUOTE = /^ {0,3}> ?/;
const LIST = /^( {0,3})(?:[-*+]|(\d{1,9})[.)])(?=[ \t]|$)/;
const TASK = /^\[([ xX])\](?=[ \t]|$)/;
const FENCE = /^( {0,3})(`{3,}|~{3,})(.*)$/;
const ESCAPABLE = '!"#$%&\'()*+,-./:;<=>?@[\\]^_`{|}~';
const SPECIAL = /[\\\n`*_~![\]]/g;

const blank = (line: string) => line.trim() === '';
const indentOf = (line: string) => /^ */.exec(line)![0].length;
const isSpace = (c: string) => /\s/.test(c);
const isPunct = (c: string) => /[\p{P}\p{S}]/u.test(c);
const unescape = (s: string) => s.replace(/\\(.)/g, (m: string, c: string) => (ESCAPABLE.includes(c) ? c : m));

// Leading tabs become spaces to the next stop of 4 columns.
function detab(line: string): string {
  const lead = /^[ \t]*/.exec(line)![0];
  if (!lead.includes('\t')) return line;
  let col = 0;
  for (const c of lead) col = c === '\t' ? col + 4 - (col % 4) : col + 1;
  return ' '.repeat(col) + line.slice(lead.length);
}

// The line without `n` leading spaces at most.
function dedent(line: string, n: number): string {
  let k = 0;
  while (k < n && line[k] === ' ') k++;
  return line.slice(k);
}

function isRule(line: string): boolean {
  let ch = '';
  let count = 0;
  for (let k = 0; k < line.length; k++) {
    const c = line[k];
    if (c === ' ' || c === '\t') continue;
    if ((count === 0 && k > 3) || (c !== '-' && c !== '*' && c !== '_') || (ch && c !== ch)) return false;
    ch = c;
    count++;
  }
  return count >= 3;
}

interface Fence {
  indent: number;
  ch: string;
  len: number;
  lang: string;
}

function fenceOpen(line: string): Fence | null {
  const m = FENCE.exec(line);
  if (!m || (m[2][0] === '`' && m[3].includes('`'))) return null;
  return { indent: m[1].length, ch: m[2][0], len: m[2].length, lang: /^\S*/.exec(m[3].trim())![0] };
}

function fenceClose(line: string, f: Fence): boolean {
  const s = line.trimEnd();
  const from = indentOf(s);
  let end = from;
  while (s[end] === f.ch) end++;
  return from <= 3 && end === s.length && end - from >= f.len;
}

interface Marker {
  indent: number;
  ordered: boolean;
  start: number;
  // The column where the item text starts.
  offset: number;
  empty: boolean;
}

function marker(line: string): Marker | null {
  const m = LIST.exec(line);
  if (!m) return null;
  const end = m[0].length;
  let sp = 0;
  while (line[end + sp] === ' ' || line[end + sp] === '\t') sp++;
  const empty = end + sp === line.length;
  const ordered = m[2] !== undefined;
  return { indent: m[1].length, ordered, start: ordered ? Number(m[2]) : 1, offset: end + (empty || sp > 4 ? 1 : sp), empty };
}

// Whether the line ends a paragraph: CommonMark lets only an ordered list from 1 do it.
function interrupts(line: string, depth: number): boolean {
  if (fenceOpen(line) || HEADING.test(line) || isRule(line)) return true;
  if (depth >= MAX_DEPTH) return false;
  if (QUOTE.test(line)) return true;
  const mk = marker(line);
  return mk !== null && !mk.empty && (!mk.ordered || mk.start === 1);
}

// The cells of a table row. A pipe after a backslash stays in the cell.
function cells(line: string): string[] {
  let s = line.trim();
  if (s.startsWith('|')) s = s.slice(1);
  const out: string[] = [];
  let from = 0;
  for (let k = 0; k < s.length; k++) {
    if (s[k] === '\\') k++;
    else if (s[k] === '|') {
      out.push(s.slice(from, k));
      from = k + 1;
    }
  }
  if (from < s.length || out.length === 0) out.push(s.slice(from));
  return out.map((c) => c.trim().replace(/\\\|/g, '|'));
}

function delimiterRow(line: string): Align[] | null {
  const t = line.trim();
  if (!/^[|:\- \t]+$/.test(t) || !t.includes('|') || !t.includes('-')) return null;
  const out: Align[] = [];
  for (const c of cells(t)) {
    if (!/^:?-+:?$/.test(c)) return null;
    const left = c.startsWith(':');
    const right = c.endsWith(':');
    out.push(left && right ? 'center' : right ? 'right' : left ? 'left' : null);
  }
  return out;
}

// The column alignment when a GFM table starts at line i, or null.
function tableAt(lines: string[], i: number): Align[] | null {
  if (i + 1 >= lines.length || !lines[i].includes('|')) return null;
  const align = delimiterRow(lines[i + 1]);
  return align && align.length === cells(lines[i]).length ? align : null;
}

function blocks(lines: string[], depth: number): Block[] {
  const out: Block[] = [];
  let i = 0;
  while (i < lines.length) {
    const line = lines[i];
    if (blank(line)) {
      i++;
      continue;
    }
    const fence = fenceOpen(line);
    if (fence) {
      const body: string[] = [];
      for (i++; i < lines.length && !fenceClose(lines[i], fence); i++) body.push(dedent(lines[i], fence.indent));
      i++;
      out.push({ type: 'code', lang: fence.lang, text: body.join('\n') });
      continue;
    }
    const h = HEADING.exec(line);
    if (h) {
      out.push({ type: 'heading', level: h[1].length, content: inline(headingText(line.slice(h[0].length))) });
      i++;
      continue;
    }
    if (isRule(line)) {
      out.push({ type: 'rule' });
      i++;
      continue;
    }
    if (depth < MAX_DEPTH && QUOTE.test(line)) {
      const body: string[] = [];
      for (; i < lines.length && QUOTE.test(lines[i]); i++) body.push(lines[i].replace(QUOTE, ''));
      out.push({ type: 'quote', blocks: blocks(body, depth + 1) });
      continue;
    }
    if (depth < MAX_DEPTH && marker(line)) {
      i = list(lines, i, depth, out);
      continue;
    }
    const align = tableAt(lines, i);
    if (align) {
      i = table(lines, i, align, depth, out);
      continue;
    }
    const text = [line.trimStart()];
    for (i++; i < lines.length && !blank(lines[i]) && !interrupts(lines[i], depth) && !tableAt(lines, i); i++) {
      text.push(lines[i].trimStart());
    }
    out.push({ type: 'paragraph', content: inline(text.join('\n').trimEnd()) });
  }
  return out;
}

// The heading text without the optional closing run of `#`.
function headingText(rest: string): string {
  const s = rest.trim();
  let end = s.length;
  while (end > 0 && s[end - 1] === '#') end--;
  return end === 0 || s[end - 1] === ' ' || s[end - 1] === '\t' ? s.slice(0, end).trimEnd() : s;
}

// A line that is indented more than the marker of its item belongs to that item.
function list(lines: string[], i: number, depth: number, out: Block[]): number {
  const first = marker(lines[i])!;
  const items: ListItem[] = [];
  for (;;) {
    let j = i;
    while (j < lines.length && blank(lines[j])) j++;
    const mk = j < lines.length && !isRule(lines[j]) ? marker(lines[j]) : null;
    if (!mk || mk.ordered !== first.ordered) break;
    const body = [lines[j].slice(mk.offset)];
    for (i = j + 1; i < lines.length; i++) {
      if (blank(lines[i])) {
        let k = i + 1;
        while (k < lines.length && blank(lines[k])) k++;
        if (k === lines.length || indentOf(lines[k]) <= mk.indent) break;
        for (; i < k; i++) body.push('');
      }
      if (indentOf(lines[i]) <= mk.indent) break;
      body.push(dedent(lines[i], mk.offset));
    }
    const task = TASK.exec(body[0]);
    if (task) body[0] = body[0].slice(3).trimStart();
    items.push({ checked: task ? task[1] !== ' ' : null, blocks: blocks(body, depth + 1) });
  }
  out.push({ type: 'list', ordered: first.ordered, start: first.start, items });
  return i;
}

// A table ends at a blank line or at the start of another block. A short row gets empty cells.
function table(lines: string[], i: number, align: Align[], depth: number, out: Block[]): number {
  const head = cells(lines[i]).map(inline);
  const rows: Inline[][][] = [];
  for (i += 2; i < lines.length && !blank(lines[i]) && !interrupts(lines[i], depth); i++) {
    const row = cells(lines[i]);
    rows.push(align.map((_, k) => inline(k < row.length ? row[k] : '')));
  }
  out.push({ type: 'table', align, head, rows });
  return i;
}

// A delimiter run of `*`, `_` or `~`, in a linked stack as in the CommonMark algorithm.
interface Delim {
  kind: 'delim';
  ch: string;
  len: number;
  orig: number;
  pos: number;
  canOpen: boolean;
  canClose: boolean;
  opens: Mark[];
  closes: Mark[];
  prev: Delim | null;
  next: Delim | null;
}

type Tok =
  | { kind: 'text'; text: string }
  | { kind: 'code'; text: string }
  | { kind: 'close'; text: string }
  | { kind: 'br' }
  | { kind: 'open'; image: boolean }
  | Delim;

// Whether the opening run `o` can pair with the closing run `c`: the CommonMark rule of 3, and
// GFM tildes of the same length.
function pairs(o: Delim, c: Delim): boolean {
  if (o.ch !== c.ch || !o.canOpen) return false;
  if (c.ch === '~') return o.len === c.len;
  return !((o.canClose || c.canOpen) && c.orig % 3 !== 0 && (o.orig + c.orig) % 3 === 0);
}

// The start of each backtick run by its length. The closing run of a code span is the next run of
// the same length, and the lookups move forward only, so all of them take linear time.
function backtickRuns(src: string): (len: number, from: number) => number {
  const at = new Map<number, number[]>();
  for (let k = src.indexOf('`'); k >= 0; ) {
    let end = k;
    while (src[end] === '`') end++;
    const list = at.get(end - k);
    if (list) list.push(k);
    else at.set(end - k, [k]);
    k = src.indexOf('`', end);
  }
  const next = new Map<number, number>();
  return (len, from) => {
    const list = at.get(len) ?? [];
    let p = next.get(len) ?? 0;
    while (p < list.length && list[p] < from) p++;
    next.set(len, p);
    return p < list.length ? list[p] : -1;
  };
}

// The target after `]`: `(url)` or `(url "title")`. The title is not kept.
function linkTarget(src: string, p: number): { url: string; end: number } | null {
  if (src[p] !== '(') return null;
  const end = Math.min(src.length, p + MAX_TARGET);
  let k = p + 1;
  const skip = () => {
    while (k < end && src.charCodeAt(k) <= 32) k++;
  };
  skip();
  let url: string;
  if (src[k] === '<') {
    const from = ++k;
    for (; k < end && src[k] !== '>'; k++) {
      if (src[k] === '\n' || src[k] === '<') return null;
      if (src[k] === '\\') k++;
    }
    if (k >= end) return null;
    url = unescape(src.slice(from, k++));
  } else {
    const from = k;
    let depth = 0;
    for (; k < end && src.charCodeAt(k) > 32; k++) {
      const c = src[k];
      if (c === '\\' && ESCAPABLE.includes(src[k + 1] ?? '\n')) k++;
      // A limit on the parentheses keeps the scans of many brackets linear in total.
      else if (c === '(' && ++depth > 32) return null;
      else if (c === ')') {
        if (depth === 0) break;
        depth--;
      }
    }
    if (depth !== 0) return null;
    url = unescape(src.slice(from, k));
  }
  const afterUrl = k;
  skip();
  const q = src[k];
  if (k > afterUrl && (q === '"' || q === "'" || q === '(')) {
    const close = q === '(' ? ')' : q;
    for (k++; k < end && src[k] !== close; k++) {
      if (src[k] === '\\') k++;
      else if (src[k] === q) return null;
    }
    k++;
    skip();
  }
  return k < end && src[k] === ')' ? { url, end: k + 1 } : null;
}

/** The inline content of a paragraph, a heading or a table cell. */
function inline(src: string): Inline[] {
  const toks: Tok[] = [];
  const brackets: { at: number; image: boolean; bottom: Delim | null }[] = [];
  let top: Delim | null = null;
  // A bracket under this index cannot open a link, because links do not hold links.
  let dead = 0;
  let text = '';
  let runs: ((len: number, from: number) => number) | null = null;
  const flush = () => {
    if (text) toks.push({ kind: 'text', text });
    text = '';
  };
  const unlink = (d: Delim) => {
    if (d.prev) d.prev.next = d.next;
    if (d.next) d.next.prev = d.prev;
    else top = d.prev;
  };
  // CommonMark "process emphasis" over the delimiters above `bottom`. The floor of each kind of
  // closer keeps the search linear.
  const emphasis = (bottom: Delim | null) => {
    const floor = new Map<string, number>();
    const base = bottom ? bottom.pos : -1;
    let closer: Delim | null = null;
    for (let d = top; d && d !== bottom; d = d.prev) closer = d;
    while (closer) {
      const c: Delim = closer;
      if (!c.canClose) {
        closer = c.next;
        continue;
      }
      const key = c.ch === '~' ? `~${c.orig}` : `${c.ch}${c.canOpen}${c.orig % 3}`;
      const stop = floor.get(key) ?? base;
      let o = c.prev;
      while (o && o.pos > stop && !pairs(o, c)) o = o.prev;
      if (o && o.pos > stop) {
        const use = c.ch === '~' ? c.len : c.len >= 2 && o.len >= 2 ? 2 : 1;
        const mark: Mark = c.ch === '~' ? 'del' : use === 2 ? 'strong' : 'em';
        o.len -= use;
        c.len -= use;
        o.opens.push(mark);
        c.closes.push(mark);
        o.next = c;
        c.prev = o;
        if (o.len === 0) unlink(o);
        if (c.len === 0) {
          closer = c.next;
          unlink(c);
        }
      } else {
        floor.set(key, c.prev ? c.prev.pos : -1);
        closer = c.next;
        if (!c.canOpen) unlink(c);
      }
    }
    while (top && top !== bottom) unlink(top);
  };

  const n = src.length;
  let i = 0;
  while (i < n) {
    SPECIAL.lastIndex = i;
    const m = SPECIAL.exec(src);
    const at = m ? m.index : n;
    // The spaces before a line break never go into the text, so the text is not cut again.
    let keep = at;
    if (src[at] === '\n') while (keep > i && src[keep - 1] === ' ') keep--;
    text += src.slice(i, keep);
    i = at;
    if (i >= n) break;
    const c = src[i];
    if (c === '\\') {
      const d = src[i + 1] ?? '';
      if (d === '\n') {
        flush();
        toks.push({ kind: 'br' });
        i += 2;
      } else if (d && ESCAPABLE.includes(d)) {
        text += d;
        i += 2;
      } else {
        text += c;
        i++;
      }
    } else if (c === '\n') {
      if (at - keep >= 2) {
        flush();
        toks.push({ kind: 'br' });
      } else text += ' ';
      i++;
    } else if (c === '`') {
      let end = i;
      while (src[end] === '`') end++;
      runs ??= backtickRuns(src);
      const close = runs(end - i, end);
      if (close < 0) text += src.slice(i, end);
      else {
        flush();
        let code = src.slice(end, close).replace(/\n/g, ' ');
        if (code.length >= 2 && code[0] === ' ' && code[code.length - 1] === ' ' && /[^ ]/.test(code)) code = code.slice(1, -1);
        toks.push({ kind: 'code', text: code });
        end = close + end - i;
      }
      i = end;
    } else if (c === '*' || c === '_' || c === '~') {
      let end = i;
      while (src[end] === c) end++;
      const len = end - i;
      if (c === '~' && len > 2) text += src.slice(i, end);
      else {
        const before = i > 0 ? src[i - 1] : ' ';
        const after = end < n ? src[end] : ' ';
        const left = !isSpace(after) && (!isPunct(after) || isSpace(before) || isPunct(before));
        const right = !isSpace(before) && (!isPunct(before) || isSpace(after) || isPunct(after));
        const canOpen = c === '_' ? left && (!right || isPunct(before)) : left;
        const canClose = c === '_' ? right && (!left || isPunct(after)) : right;
        flush();
        const d: Delim = { kind: 'delim', ch: c, len, orig: len, pos: toks.length, canOpen, canClose, opens: [], closes: [], prev: null, next: null };
        toks.push(d);
        if (canOpen || canClose) {
          d.prev = top;
          if (top) top.next = d;
          top = d;
        }
      }
      i = end;
    } else if (c === '[' || (c === '!' && src[i + 1] === '[')) {
      flush();
      const image = c === '!';
      brackets.push({ at: toks.length, image, bottom: top });
      toks.push({ kind: 'open', image });
      i += image ? 2 : 1;
    } else if (c === '!') {
      text += c;
      i++;
    } else {
      const b = brackets.pop();
      const active = b !== undefined && (b.image || brackets.length >= dead);
      dead = Math.min(dead, brackets.length);
      const target = b && active ? linkTarget(src, i + 1) : null;
      if (b && target) {
        flush();
        emphasis(b.bottom);
        toks.push({ kind: 'close', text: target.url });
        if (!b.image) dead = brackets.length;
        i = target.end;
      } else {
        if (b) toks[b.at] = { kind: 'text', text: b.image ? '![' : '[' };
        text += c;
        i++;
      }
    }
  }
  flush();
  for (const b of brackets) toks[b.at] = { kind: 'text', text: b.image ? '![' : '[' };
  emphasis(null);
  return build(toks);
}

interface Frame {
  kind: Mark | 'link' | 'image';
  kids: Inline[];
  // A frame deeper than MAX_INLINE adds its children to its parent.
  flat: boolean;
}

// The inline tree of the tokens. The matched delimiters nest, so a stack of frames builds it.
function build(toks: Tok[]): Inline[] {
  const root: Inline[] = [];
  const stack: Frame[] = [];
  const kids = () => (stack.length ? stack[stack.length - 1].kids : root);
  const add = (s: string) => {
    const k = kids();
    const last = k[k.length - 1];
    if (!s) return;
    if (last && last.type === 'text') last.text += s;
    else k.push({ type: 'text', text: s });
  };
  const open = (kind: Frame['kind']) => {
    const flat = stack.length >= MAX_INLINE;
    stack.push({ kind, kids: flat ? kids() : [], flat });
  };
  const close = (url: string) => {
    const f = stack.pop();
    if (!f || f.flat) return;
    const k = kids();
    if (f.kind === 'link') k.push({ type: 'link', url, children: f.kids });
    else if (f.kind === 'image') k.push({ type: 'image', src: url, alt: plain(f.kids) });
    else k.push({ type: f.kind, children: f.kids });
  };
  for (const t of toks) {
    if (t.kind === 'text') add(t.text);
    else if (t.kind === 'code') kids().push({ type: 'code', text: t.text });
    else if (t.kind === 'br') kids().push({ type: 'br' });
    else if (t.kind === 'open') open(t.image ? 'image' : 'link');
    else if (t.kind === 'close') close(t.text);
    else {
      for (let k = 0; k < t.closes.length; k++) close('');
      add(t.ch.repeat(t.len));
      for (let k = t.opens.length - 1; k >= 0; k--) open(t.opens[k]);
    }
  }
  while (stack.length) close('');
  return root;
}

function plain(nodes: Inline[]): string {
  let s = '';
  for (const n of nodes) {
    s += n.type === 'text' || n.type === 'code' ? n.text : n.type === 'br' ? ' ' : n.type === 'image' ? n.alt : plain(n.children);
  }
  return s;
}

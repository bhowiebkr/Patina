// The Logs of the editor and the Output of the nodes (spec 15.8). Each is a panel of the dock (15.2).

import { useEffect, useRef } from 'react';
import type { Reveal } from '../code/reveal';
import { highlight, type Part } from './highlight';
import { hintOf } from './hints';

export interface Line {
  text: string;
  kind: 'out' | 'err' | 'warn' | 'info';
  /** The place in the code of a compile error. A click shows it. */
  at?: Reveal;
}

/** A panel keeps the newest lines only. */
const MAX_LINES = 5000;

export function addLines(list: Line[], more: Line[]): Line[] {
  const all = list.concat(more);
  return all.length > MAX_LINES ? all.slice(all.length - MAX_LINES) : all;
}

/** Adds a "Fix:" line after each error or warning line that the list of common errors knows, one time for each fix. */
export function withHints(lines: Line[]): Line[] {
  const given = new Set<string>();
  return lines.flatMap((line) => {
    const hint = line.kind === 'err' || line.kind === 'warn' || line.kind === 'out' ? hintOf(line.text) : null;
    if (!hint || given.has(hint)) return [line];
    given.add(hint);
    return [line, { text: `Fix: ${hint}`, kind: 'info' as const }];
  });
}

// Each line is highlighted one time.
const parts = new WeakMap<Line, Part[]>();

function Text({ line }: { line: Line }) {
  let p = parts.get(line);
  if (!p) parts.set(line, (p = highlight(line.text)));
  return p.map(([text, kind], i) => (kind ? <span key={i} className={`hl-${kind}`}>{text}</span> : text));
}

export function Lines({ lines, label, onPick }: { lines: Line[]; label: string; onPick: (at: Reveal) => void }) {
  const box = useRef<HTMLPreElement>(null);
  useEffect(() => {
    if (box.current) box.current.scrollTop = box.current.scrollHeight;
  }, [lines]);
  return (
    <pre ref={box} className="lines" role="log" aria-label={label}>
      {lines.map((line, i) =>
        line.at ? (
          <button key={i} className={`line-link ${line.kind}`} onClick={() => onPick({ ...line.at!, n: Date.now() })}>
            <Text line={line} />
          </button>
        ) : (
          <div key={i} className={line.kind}>
            <Text line={line} />
          </div>
        ),
      )}
    </pre>
  );
}

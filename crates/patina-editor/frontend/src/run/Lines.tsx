// The Logs of the editor and the Output of the nodes (spec 15.8). Each is a panel of the dock (15.2).

import { useEffect, useRef } from 'react';
import type { Reveal } from '../code/CodeTab';

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
            {line.text}
          </button>
        ) : (
          <div key={i} className={line.kind}>
            {line.text}
          </div>
        ),
      )}
    </pre>
  );
}

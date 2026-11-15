// The bottom panel (spec 15.8): the Logs of the editor and the Output of the nodes.

import { useEffect, useRef, useState } from 'react';
import type { Reveal } from './CodeTab';

export interface Line {
  text: string;
  kind: 'out' | 'err' | 'info';
  /** The place in the code of a compile error. A click shows it. */
  at?: Reveal;
}

/** The panel keeps the newest lines only. */
const MAX_LINES = 5000;

export function addLines(list: Line[], more: Line[]): Line[] {
  const all = list.concat(more);
  return all.length > MAX_LINES ? all.slice(all.length - MAX_LINES) : all;
}

type Tab = 'output' | 'logs';

interface Props {
  output: Line[];
  logs: Line[];
  onClear: (tab: Tab) => void;
  onPick: (at: Reveal) => void;
}

export function BottomPanel({ output, logs, onClear, onPick }: Props) {
  const [tab, setTab] = useState<Tab>('output');
  const lines = tab === 'output' ? output : logs;
  const box = useRef<HTMLPreElement>(null);
  useEffect(() => {
    if (box.current) box.current.scrollTop = box.current.scrollHeight;
  }, [lines]);
  return (
    <section className="bottom" aria-label="Bottom panel">
      <div className="tabs" role="tablist">
        <button role="tab" aria-selected={tab === 'output'} className={tab === 'output' ? 'tab active' : 'tab'} onClick={() => setTab('output')}>
          Output
        </button>
        <button role="tab" aria-selected={tab === 'logs'} className={tab === 'logs' ? 'tab active' : 'tab'} onClick={() => setTab('logs')}>
          Logs
        </button>
        <span className="spacer" />
        <button className="tab" onClick={() => onClear(tab)}>
          Clear
        </button>
      </div>
      <pre ref={box} className="lines" role="log">
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
    </section>
  );
}

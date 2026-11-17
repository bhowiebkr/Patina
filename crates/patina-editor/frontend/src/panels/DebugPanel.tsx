import { useMemo } from 'react';
import { duration, type Domain, type NodeTime } from '../api';
import { lanes } from './timeline';

const TITLES: Record<Domain, string> = { frame: 'Frame', audio: 'Audio', event: 'Event', batch: 'Batch' };

interface Props {
  times: ReadonlyMap<string, NodeTime>;
  domains: Readonly<Record<string, Domain>>;
  frameRate: number;
  /** The folder name of each node of the open graph, by ID. */
  names: ReadonlyMap<string, string>;
  onSelect: (ids: string[]) => void;
}

/** The **Debug** tab: a timeline of the node calls of each domain and thread in the last run (15.7). */
export function DebugPanel({ times, domains, frameRate, names, onSelect }: Props) {
  const all = useMemo(() => lanes(times.values(), domains, frameRate), [times, domains, frameRate]);
  if (all.length === 0) return <p className="debug-empty">Run the graph to see the node calls of each thread.</p>;
  return (
    <div className="debug">
      {all.map((lane) => {
        const scale = Math.max(lane.span, lane.limit ?? 0) || 1;
        const pct = (ms: number) => `${(ms / scale) * 100}%`;
        const over = lane.limit !== null && lane.span > lane.limit;
        const limitName = lane.domain === 'audio' ? 'block deadline' : 'frame budget';
        return (
          <section key={lane.domain} className="debug-lane" aria-label={`${TITLES[lane.domain]} timeline`}>
            <header>
              <b>{TITLES[lane.domain]}</b> {duration(lane.span)}
              {lane.limit !== null && (
                <span className={over ? 'debug-over' : 'debug-limit-text'}>
                  {over ? `Over the ${limitName} of ${duration(lane.limit)}` : `${limitName[0].toUpperCase()}${limitName.slice(1)} ${duration(lane.limit)}`}
                </span>
              )}
            </header>
            {lane.rows.map((row) => (
              <div key={row.thread} className="debug-row">
                <span className="debug-thread">Thread {row.thread}</span>
                <div className="debug-track">
                  {row.calls.map((c) => {
                    const name = names.get(c.id);
                    return (
                      <button
                        key={c.id}
                        type="button"
                        className="debug-call"
                        style={{ left: pct(c.start), width: pct(c.ms) }}
                        title={`${name ?? c.id}: ${duration(c.ms)}, starts at ${duration(c.start)}`}
                        disabled={name === undefined}
                        onClick={() => onSelect([c.id])}
                      >
                        {name ?? c.id}
                      </button>
                    );
                  })}
                  {lane.limit !== null && <div className="debug-limit" style={{ left: pct(lane.limit) }} />}
                </div>
              </div>
            ))}
          </section>
        );
      })}
    </div>
  );
}

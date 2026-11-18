// The path bar of a subgraph: the trail of the open graphs and the Test switch (15.9).
import { Fragment } from 'react';
import { api, type View } from '../api';
import type { Crumb } from './crumbs';

export function GraphPath({ view, path, testing, act, onTest, onOpen, onGo }: {
  view: View;
  path: Crumb[];
  testing: boolean;
  act: (job: Promise<View>) => void;
  onTest: () => void;
  onOpen: (name: string) => void;
  onGo: (name: string, next: (trail: Crumb[]) => Crumb[]) => void;
}) {
  return (
    <nav className="graph-path" aria-label="Graph path">
      <button className="btn" onClick={() => onOpen(view.top)}>
        {view.top}
      </button>
      {path.map((c, i) =>
        i < path.length - 1 ? (
          <Fragment key={i}>
            <span>/</span>
            <button className="btn" title={`Open ${c.graph}`} onClick={() => onGo(c.graph, () => path.slice(0, i + 1))}>
              {c.label}
            </button>
          </Fragment>
        ) : (
          <span key={i} aria-current="page" title={c.label === c.graph ? undefined : c.graph}>
            / {c.label}
          </span>
        ),
      )}
      <button
        className={testing ? 'btn primary' : 'btn'}
        aria-pressed={testing}
        disabled={!view.trusted}
        onClick={() => (testing ? act(api.test(null)) : onTest())}
        title={testing ? 'End the test. The top graph runs again.' : 'Run this subgraph alone. Click an input switch to change it.'}
      >
        {testing ? 'End test' : 'Test'}
      </button>
    </nav>
  );
}

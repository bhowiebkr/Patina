// The View panel (spec 15.8): the widgets of a project view. The panel asks the view for its widget
// lines at the display rate while it shows, and sends the user input with the next ask.

import { useEffect, useRef, useState } from 'react';
import { api, type View } from './api';
import { GuiWidgets } from './NodeGui';

interface Props {
  /** The view folder `views/<name>`, or null when no view is open. */
  name: string | null;
  /** The code revision of the project. A code change builds the view again. */
  revision: number;
  trusted: boolean;
  /** Shows the project after a write of the view to the graph. */
  onView: (view: View) => void;
}

export function ViewPanel({ name, revision, trusted, onView }: Props) {
  const [tree, setTree] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [attempt, setAttempt] = useState(0);
  const events = useRef<string[]>([]);
  const show = useRef(onView);
  show.current = onView;

  useEffect(() => {
    if (!name || !trusted) return;
    let live = true;
    let frame = 0;
    setTree(null);
    setError(null);
    const ask = () => {
      const lines = events.current.splice(0).join('\n');
      api.viewUi(name, lines).then(
        (f) => {
          if (!live) return;
          setTree(f.tree);
          if (f.view) show.current(f.view);
          frame = requestAnimationFrame(ask);
        },
        // A failed build stops the asks, so the view does not build at each frame.
        (e) => live && setError(String(e)),
      );
    };
    ask();
    return () => {
      live = false;
      cancelAnimationFrame(frame);
    };
  }, [name, revision, trusted, attempt]);

  let body;
  if (!name) body = <div className="item empty">Open a view from the Views menu.</div>;
  else if (!trusted) body = <div className="item empty">Trust the project to open the view.</div>;
  else if (error)
    body = (
      <>
        <div className="item error-text" role="alert">
          {error}
        </div>
        <button className="btn" onClick={() => setAttempt((n) => n + 1)}>
          Try again
        </button>
      </>
    );
  else if (tree === null) body = <div className="item empty">Building the view</div>;
  else body = <GuiWidgets tree={tree} disabled={false} label="View controls" send={(line) => events.current.push(line)} />;
  return (
    <aside className="panel" aria-label="View">
      {name && <div className="panel-title">{name}</div>}
      {body}
    </aside>
  );
}

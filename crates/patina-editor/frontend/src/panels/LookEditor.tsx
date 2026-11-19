// The outline editor of a custom subgraph symbol (4.3, 15.9): the symbol with a handle on each
// corner and on the middle of each edge. A drag moves a corner, and a drag on a middle handle adds
// a corner there. One release or one key press is one edit, so one undo.

import { useRef, useState, type KeyboardEvent, type PointerEvent } from 'react';
import type { Point } from '../api';
import { SymbolBody } from '../canvas/SymbolView';
import { OUTLINE_MAX, type SymbolLayout } from '../canvas/symbols';

/** The grid of a corner, in symbol pixels. */
const SNAP = 2;
const MIN_CORNERS = 3;
const MAX_CORNERS = 32;
const snap = (v: number) => Math.min(OUTLINE_MAX, Math.max(0, Math.round(v / SNAP) * SNAP));

interface Drag {
  index: number;
  from: Point;
  at: Point;
  /** The symbol pixels for each screen pixel. */
  scale: number;
}

/** The outline editor. `layoutOf` gives the layout of an outline, so that the symbol follows a drag. */
export function LookEditor({ outline, layoutOf, disabled, onChange }: { outline: Point[]; layoutOf: (outline: Point[]) => SymbolLayout; disabled: boolean; onChange: (outline: Point[]) => void }) {
  // The draft of a drag belongs to the outline that it started from, so a new outline from the engine replaces it.
  const [draft, setDraft] = useState<{ base: Point[]; points: Point[] } | null>(null);
  const drag = useRef<Drag | null>(null);
  const shown = draft && draft.base === outline ? draft.points : outline;
  const layout = layoutOf(shown);
  const [ox, oy] = layout.origin ?? [0, 0];

  const start = (e: PointerEvent<SVGElement>, index: number, points: Point[]) => {
    if (disabled || e.button !== 0) return;
    e.preventDefault();
    const svg = e.currentTarget.ownerSVGElement!;
    svg.setPointerCapture(e.pointerId);
    drag.current = { index, from: points[index], at: [e.clientX, e.clientY], scale: layout.width / svg.getBoundingClientRect().width };
    setDraft({ base: outline, points });
  };
  const move = (e: PointerEvent<SVGSVGElement>) => {
    const d = drag.current;
    if (!d || !draft) return;
    const p: Point = [snap(d.from[0] + (e.clientX - d.at[0]) * d.scale), snap(d.from[1] + (e.clientY - d.at[1]) * d.scale)];
    setDraft({ ...draft, points: draft.points.map((q, i) => (i === d.index ? p : q)) });
  };
  const end = () => {
    if (!drag.current || !draft) return;
    drag.current = null;
    const same = draft.points.length === outline.length && draft.points.every((p, i) => p[0] === outline[i][0] && p[1] === outline[i][1]);
    if (same) setDraft(null);
    else onChange(draft.points);
  };
  const remove = (i: number) => {
    if (!disabled && outline.length > MIN_CORNERS) onChange(outline.filter((_, j) => j !== i));
  };
  const keys = (e: KeyboardEvent<SVGElement>, i: number) => {
    const step = e.shiftKey ? 10 : SNAP;
    const by: Record<string, Point> = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step] };
    if (e.key === 'Delete' || e.key === 'Backspace') remove(i);
    else if (by[e.key] && !disabled) {
      const [x, y] = outline[i];
      const p: Point = [snap(x + by[e.key][0]), snap(y + by[e.key][1])];
      if (p[0] !== x || p[1] !== y) onChange(outline.map((q, j) => (j === i ? p : q)));
    } else return;
    e.preventDefault();
  };

  return (
    <div className="look-editor">
      <SymbolBody layout={layout} far={false} />
      <svg className="look-handles" width={layout.width} height={layout.height} viewBox={`0 0 ${layout.width} ${layout.height}`} onPointerMove={move} onPointerUp={end} onPointerCancel={end}>
        {shown.length < MAX_CORNERS &&
          shown.map((p, i) => {
            const q = shown[(i + 1) % shown.length];
            const mid: Point = [snap((p[0] + q[0]) / 2), snap((p[1] + q[1]) / 2)];
            return (
              <circle
                key={`m${i}`}
                className="look-add"
                cx={ox + (p[0] + q[0]) / 2}
                cy={oy + (p[1] + q[1]) / 2}
                r={3}
                onPointerDown={(e) => start(e, i + 1, [...shown.slice(0, i + 1), mid, ...shown.slice(i + 1)])}
              >
                <title>Drag to add a corner</title>
              </circle>
            );
          })}
        {shown.map(([x, y], i) => (
          <rect
            key={`c${i}`}
            className="look-corner"
            x={ox + x - 4}
            y={oy + y - 4}
            width={8}
            height={8}
            tabIndex={disabled ? -1 : 0}
            role="button"
            aria-label={`Corner ${i + 1} at ${x}, ${y}`}
            onPointerDown={(e) => start(e, i, shown)}
            onDoubleClick={() => remove(i)}
            onKeyDown={(e) => keys(e, i)}
          >
            <title>Drag to move. Double-click or press Delete to remove.</title>
          </rect>
        ))}
      </svg>
    </div>
  );
}

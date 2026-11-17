// The handles of the selected wire (spec 15.9): a square on each route point, and a bar in each
// middle segment of an orthogonal wire.

import { useRef, useState, type KeyboardEvent, type PointerEvent } from 'react';
import { useReactFlow, useStore, ViewportPortal } from '@xyflow/react';
import type { Point } from './api';
import { moveSegment, segments } from './wires';

/** The size of a route handle, in screen pixels. */
const HANDLE = 10;

interface HandleProps {
  route: readonly Point[];
  dragging: boolean;
  setDraft: (route: readonly Point[] | null) => void;
  send: (points: Point[]) => Promise<void>;
}

/** The square handles of the route points of the selected wire, above the nodes. */
export function RouteHandles({ route, dragging, setDraft, send }: HandleProps) {
  const zoom = useStore((s) => s.transform[2]);
  const flow = useReactFlow();
  const drag = useRef<{ k: number; dx: number; dy: number; route: Point[] | null } | null>(null);

  const down = (k: number, e: PointerEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    e.stopPropagation();
    e.currentTarget.setPointerCapture(e.pointerId);
    const p = flow.screenToFlowPosition({ x: e.clientX, y: e.clientY });
    drag.current = { k, dx: route[k][0] - p.x, dy: route[k][1] - p.y, route: null };
  };
  const move = (e: PointerEvent) => {
    const g = drag.current;
    if (!g) return;
    const p = flow.screenToFlowPosition({ x: e.clientX, y: e.clientY });
    const point: Point = [Math.round(p.x + g.dx), Math.round(p.y + g.dy)];
    g.route = route.map((q, i) => (i === g.k ? point : q));
    setDraft(g.route);
  };
  const up = () => {
    const g = drag.current;
    drag.current = null;
    if (g?.route) void send(g.route).finally(() => setDraft(null));
  };
  const remove = (k: number) => void send(route.filter((_, i) => i !== k));
  const key = (k: number, e: KeyboardEvent) => {
    const step = e.shiftKey ? 1 : 10;
    const by: Record<string, Point> = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step] };
    if (e.key === 'Escape' && drag.current) {
      drag.current = null;
      setDraft(null);
    } else if ((e.key === 'Delete' || e.key === 'Backspace') && !drag.current) {
      remove(k);
    } else if (by[e.key] && !drag.current) {
      const [dx, dy] = by[e.key];
      void send(route.map((q, i) => (i === k ? [q[0] + dx, q[1] + dy] : q)));
    } else {
      return;
    }
    // The key acts on the point only: the canvas does not also disconnect the wire or clear the selection.
    e.preventDefault();
    e.stopPropagation();
  };

  return (
    <ViewportPortal>
      {route.map(([x, y], k) => (
        <div
          key={k}
          className={dragging ? 'route-point nodrag nopan dragging' : 'route-point nodrag nopan'}
          style={{ transform: `translate(${x - HANDLE / 2}px, ${y - HANDLE / 2}px) scale(${1 / zoom})` }}
          role="button"
          tabIndex={0}
          aria-label={`Route point ${k + 1}`}
          title="Drag to move. Delete or a double-click removes the point."
          onPointerDown={(e) => down(k, e)}
          onPointerMove={move}
          onPointerUp={up}
          onDoubleClick={(e) => {
            e.stopPropagation();
            remove(k);
          }}
          onKeyDown={(e) => key(k, e)}
        />
      ))}
    </ViewportPortal>
  );
}

interface SegmentProps {
  ends: [number, number, number, number];
  route: readonly Point[];
  setDraft: (route: readonly Point[] | null) => void;
  send: (points: Point[]) => Promise<void>;
}

/** The bar handles in the middle segments of a selected orthogonal wire. A drag moves a segment sideways (15.9). */
export function SegmentHandles({ ends, route, setDraft, send }: SegmentProps) {
  const zoom = useStore((s) => s.transform[2]);
  const flow = useReactFlow();
  // The route at the start of the drag: the segment indices refer to it until the drag ends. The
  // handlers read the ref, because the next pointer event can come before React renders the state.
  type Drag = { k: number; start: number; d: number; base: readonly Point[] };
  const dragRef = useRef<Drag | null>(null);
  const [drag, setDragState] = useState<Drag | null>(null);
  const setDrag = (g: Drag | null) => {
    dragRef.current = g;
    setDragState(g);
  };
  const [sx, sy, tx, ty] = ends;
  const list = segments(sx, sy, tx, ty, drag?.base ?? route);
  const along = (e: PointerEvent, vertical: boolean) => {
    const p = flow.screenToFlowPosition({ x: e.clientX, y: e.clientY });
    return vertical ? p.x : p.y;
  };
  const down = (k: number, vertical: boolean, e: PointerEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    e.stopPropagation();
    e.currentTarget.setPointerCapture(e.pointerId);
    setDrag({ k, start: along(e, vertical), d: 0, base: route });
  };
  const move = (vertical: boolean, e: PointerEvent) => {
    const g = dragRef.current;
    if (!g) return;
    const d = Math.round(along(e, vertical) - g.start);
    setDrag({ ...g, d });
    setDraft(moveSegment(sx, sy, tx, ty, g.base, g.k, d));
  };
  const up = () => {
    const g = dragRef.current;
    if (g && g.d !== 0) void send(moveSegment(sx, sy, tx, ty, g.base, g.k, g.d)).finally(() => setDraft(null));
    setDrag(null);
  };
  const key = (k: number, vertical: boolean, e: KeyboardEvent) => {
    const step = e.shiftKey ? 1 : 10;
    const by: Record<string, number> = vertical ? { ArrowLeft: -step, ArrowRight: step } : { ArrowUp: -step, ArrowDown: step };
    if (e.key === 'Escape' && dragRef.current) {
      setDrag(null);
      setDraft(null);
    } else if (by[e.key] && !dragRef.current) {
      void send(moveSegment(sx, sy, tx, ty, route, k, by[e.key]));
    } else {
      return;
    }
    e.preventDefault();
    e.stopPropagation();
  };

  return (
    <ViewportPortal>
      {list.map(({ k, x, y, vertical }) => {
        if (drag && drag.k !== k) return null;
        const [cx, cy] = drag ? (vertical ? [x + drag.d, y] : [x, y + drag.d]) : [x, y];
        return (
          <div
            key={k}
            className={`segment-handle nodrag nopan${vertical ? ' vertical' : ''}`}
            style={{ transform: `translate(${cx}px, ${cy}px) translate(-50%, -50%) scale(${1 / zoom})` }}
            role="button"
            tabIndex={0}
            aria-label={`Segment ${k}, ${vertical ? 'vertical' : 'horizontal'}`}
            title="Drag to move the segment sideways. The arrow keys move it by 10 px, or by 1 px with Shift."
            onPointerDown={(e) => down(k, vertical, e)}
            onPointerMove={(e) => move(vertical, e)}
            onPointerUp={up}
            onKeyDown={(e) => key(k, vertical, e)}
          />
        );
      })}
    </ViewportPortal>
  );
}

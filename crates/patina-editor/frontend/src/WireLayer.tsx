// The connections on the canvas (spec 15.1, 15.9). WebGL draws the visible wires under the nodes.
// Each React Flow edge keeps an invisible path on the same curve for the click, the box selection
// and the keyboard focus. Without WebGL2, CSS shows the paths of the edges instead.

import {
  createContext,
  useContext,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
  type KeyboardEvent,
  type MouseEvent,
  type PointerEvent,
} from 'react';
import { EdgeLabelRenderer, getBezierPath, Position, useReactFlow, useStore, useStoreApi, ViewportPortal, type Edge, type EdgeProps } from '@xyflow/react';
import type { Corners, Edit, KindInfo, Point, Style, Value } from './api';
import { HoverContext, PITCH, familyOf, hoveredLane, kindOf, laneAt, laneTip, wireBits } from './bus';
import { FAR_ZOOM, GlWires } from './gl';
import type { Labels } from './labels';
import type { Router } from './route';
import { anchor, insertAt, moveSegment, segments, shape, wirePath, type WireStore } from './wires';

/** The hit area of a wire, in screen pixels, at each zoom. */
const HIT_WIDTH = 12;
/** The size of a route handle, in screen pixels. */
const HANDLE = 10;

/** The ends of a wire, its route points, its look, the type of its value, its wire name and its bus name (spec 15.9). */
export type WireData = { from: string; to: string; route: readonly Point[]; style?: Style; corners?: Corners; ty?: string; fixed?: boolean; name?: string; bus?: string };

export interface WireTools {
  store: WireStore;
  /** The automatic routes of the wires with the graph default look (15.9). */
  router: Router;
  /** The wire labels that something covers (15.9). */
  labels: Labels;
  onEdit: (edit: Edit) => Promise<void>;
  editable: boolean;
  /** Each live value by output (15.9), and true when it is from an old run. */
  values: ReadonlyMap<string, Value>;
  stale: (from: string) => boolean;
  /** The kinds of the connection types, for the lanes of each bus (6.3). */
  kinds: readonly KindInfo[];
}

export const WireContext = createContext<WireTools | null>(null);

/** An order wire (9.2): a dotted SVG line that carries no data, between the headers of two nodes. */
export function OrderEdge({ sourceX, sourceY, targetX, targetY }: EdgeProps) {
  const [path] = getBezierPath({ sourceX, sourceY, sourcePosition: Position.Right, targetX, targetY, targetPosition: Position.Left });
  return (
    <>
      <path className="order-line" d={path} />
      <path className="wire-hit" d={path} />
    </>
  );
}

const CONFLICT = 'No clear path around the nodes. Move a node or a route point.';

export function WireEdge({ id, sourceX, sourceY, targetX, targetY, selected, data }: EdgeProps<Edge<WireData>>) {
  const { store, router, labels, onEdit, editable, values, stale, kinds } = useContext(WireContext)!;
  const hover = useContext(HoverContext);
  const flow = useReactFlow();
  // The route during a drag. It stays until the edit gives the new graph, so the wire does not jump back.
  const [draft, setDraft] = useState<readonly Point[] | null>(null);
  const route = draft ?? data!.route;
  const value = values.get(data!.from);
  const kind = kindOf(kinds, data!.ty);
  // A bool wire is one lane: lit when true. A kind with lanes shows its lanes before a run (15.9).
  const bits = wireBits(value, kind);
  const family = familyOf(kind);
  const old = stale(data!.from);
  const lane = useSyncExternalStore(hover.subscribe, () => hoveredLane(hover.get(), data!.from, data!.to));
  const tip = useSyncExternalStore(hover.subscribe, () => (hover.get()?.wire === id ? hover.get() : null));
  // A bus shows one line with its width at a far zoom (15.9).
  const far = useStore((s) => s.transform[2] < FAR_ZOOM);
  const { from, style, corners } = data!;
  // A wire with the graph default look goes around the node bodies. Its route points are waypoints (15.9).
  const auto = style === undefined;
  const width = Math.max(2, bits.length * PITCH);
  useLayoutEffect(() => {
    if (auto) router.setWire(id, { sx: sourceX, sy: sourceY, tx: targetX, ty: targetY, via: route, net: from, width });
    else router.removeWire(id);
  }, [router, id, auto, sourceX, sourceY, targetX, targetY, route, from, width]);
  useLayoutEffect(() => () => router.removeWire(id), [router, id]);
  const routed = useSyncExternalStore(router.subscribe, () => (auto ? router.bends(id) : undefined));
  const conflict = useSyncExternalStore(router.subscribe, () => (auto ? router.conflict(id) : undefined));
  const path = useMemo(
    () =>
      auto
        ? shape(sourceX, sourceY, targetX, targetY, routed ?? route, routed ? 'polyline' : 'orthogonal', corners, width)
        : shape(sourceX, sourceY, targetX, targetY, route, style, corners, width),
    [auto, sourceX, sourceY, targetX, targetY, routed, route, style, corners, width],
  );
  useLayoutEffect(
    () => store.set(id, sourceX, sourceY, targetX, targetY, path, !!selected, bits, old, family, lane),
    [store, id, sourceX, sourceY, targetX, targetY, path, selected, bits, old, family, lane],
  );
  useLayoutEffect(() => () => store.remove(id), [store, id]);
  const send = (points: Point[]) => onEdit({ kind: 'SetRoute', to: data!.to, points });
  const insert = (x: number, y: number) => {
    if (!editable || data!.fixed) return;
    const k = insertAt(sourceX, sourceY, targetX, targetY, route, x, y);
    void send([...route.slice(0, k), [Math.round(x), Math.round(y)], ...route.slice(k)]);
  };
  const add = (e: MouseEvent) => {
    const p = flow.screenToFlowPosition({ x: e.clientX, y: e.clientY });
    insert(p.x, p.y);
  };
  const d = wirePath(sourceX, sourceY, targetX, targetY, path);
  const [mx, my, fit] = anchor(sourceX, sourceY, targetX, targetY, path);
  // + or Insert on the focused wire adds a route point at its middle: the keyboard double-click (15.9).
  const hitRef = useRef<SVGPathElement>(null);
  useEffect(() => {
    const g = hitRef.current?.closest<SVGGElement>('.react-flow__edge');
    const key = (e: globalThis.KeyboardEvent) => {
      if (e.target !== g || (e.key !== '+' && e.key !== 'Insert')) return;
      e.preventDefault();
      insert(mx, my);
    };
    g?.addEventListener('keydown', key);
    return () => g?.removeEventListener('keydown', key);
  });
  const n = bits.length;
  const ribbon = n > 1;
  // At a far zoom, only a selected wire shows its badge and type label. A bus keeps its width (15.1, 15.9).
  const near = !far || selected;
  // A bus shows its name and width in place of the type label, above its lanes.
  const bus = data!.bus ?? kind?.label ?? value?.lanes?.label ?? data!.ty ?? 'bus';
  // One line shows its wire name in place of the type label.
  // A label with no room hides. The sockets and the pin tooltips at the two ends show the names.
  const room = selected ? Infinity : fit;
  const label = room < 24 ? undefined : ribbon ? (far ? `/ ${n}` : `${bus}[${n - 1}:0] / ${n}`) : near ? (data!.name ?? data!.ty) : undefined;
  const above = 12 + (ribbon && !far ? ((n - 1) * PITCH) / 2 : 0);
  const badge = value !== undefined && near ? value.text : undefined;
  // A badge cuts its text to the room of the label, but keeps room for a short value.
  const badgeRoom = selected ? undefined : Math.max(fit, 32);
  const named = data!.name !== undefined;
  // The boxes of the label and the badge, from the Consolas sizes of canvas.css.
  useLayoutEffect(() => {
    const w = label === undefined ? 0 : Math.min(room, label.length * 5.5);
    const bw = badge === undefined ? 0 : Math.min(badgeRoom ?? Infinity, badge.length * 6.05 + 12);
    const box = label === undefined ? null : { l: mx - w / 2, t: my - above - 13, r: mx + w / 2, b: my - above };
    const badgeBox = badge === undefined ? null : { l: mx - bw / 2, t: my - 9, r: mx + bw / 2, b: my + 9 };
    labels.set(id, box, badgeBox, selected ? -1 : ribbon ? 0 : named ? 1 : 2);
  }, [labels, id, label, badge, badgeRoom, room, mx, my, above, selected, ribbon, named]);
  useLayoutEffect(() => () => labels.remove(id), [labels, id]);
  const covered = useSyncExternalStore(labels.subscribe, () => labels.hidden(id));
  const badgeCovered = useSyncExternalStore(labels.subscribe, () => labels.badgeHidden(id));
  // The lane under the pointer (15.9). The hit area covers the full ribbon.
  const leave = () => {
    if (hover.get()?.wire === id) hover.set(null);
  };
  const move = (e: PointerEvent<SVGPathElement>) => {
    if (!ribbon || far) return;
    const p = flow.screenToFlowPosition({ x: e.clientX, y: e.clientY });
    const across = store.across(id, p.x, p.y);
    const k = across === null ? -1 : laneAt(across, n);
    if (k < 0) leave();
    else hover.set({ from: data!.from, to: data!.to, lane: k, wire: id, x: p.x, y: p.y });
  };
  const hit = ribbon && !far ? { strokeWidth: `max(var(--wire-hit, 12px), ${width + 4}px)` } : undefined;
  return (
    <>
      {/* shortcut: the SVG fallback draws a bus as one line; draw its lanes when a GPU without WebGL2 matters. */}
      <path className="wire-line" d={d} />
      <path ref={hitRef} className="wire-hit" d={d} style={hit} onDoubleClick={add} onPointerMove={move} onPointerLeave={leave} />
      <EdgeLabelRenderer>
        {tip && (
          <div className="lane-tip" style={{ transform: `translate(${tip.x}px, ${tip.y}px) translate(12px, calc(-100% - 8px))` }}>
            {laneTip(bus, kind?.lanes ?? [], bits, tip.lane, value !== undefined)}
          </div>
        )}
        {/* The type label or the bus label is above the badge (15.9). */}
        {label && !covered && (
          <div
            className={`wire-type${data!.name ? ' named' : ''}${selected ? ' selected' : ''}`}
            title={data!.ty}
            style={{ transform: `translate(${mx}px, ${my}px) translate(-50%, calc(-100% - ${above}px))`, maxWidth: selected ? undefined : room }}
          >
            {label}
          </div>
        )}
        {conflict && (
          <div
            className="route-conflict"
            role="img"
            aria-label={CONFLICT}
            title={CONFLICT}
            style={{ transform: `translate(${conflict[0]}px, ${conflict[1]}px) translate(-50%, -50%)` }}
          >
            !
          </div>
        )}
        {badge !== undefined && !badgeCovered && (
          <div
            className={`wire-value${old ? ' stale' : ''}${selected ? ' selected' : ''}`}
            style={{ transform: `translate(${mx}px, ${my}px) translate(-50%, -50%)`, maxWidth: badgeRoom }}
          >
            {badge}
          </div>
        )}
      </EdgeLabelRenderer>
      {selected && editable && <RouteHandles route={route} dragging={draft !== null} setDraft={setDraft} send={send} />}
      {selected && editable && style === 'orthogonal' && !data!.fixed && (
        <SegmentHandles ends={[sourceX, sourceY, targetX, targetY]} route={route} setDraft={setDraft} send={send} />
      )}
    </>
  );
}

interface HandleProps {
  route: readonly Point[];
  dragging: boolean;
  setDraft: (route: readonly Point[] | null) => void;
  send: (points: Point[]) => Promise<void>;
}

/** The square handles of the route points of the selected wire, above the nodes. */
function RouteHandles({ route, dragging, setDraft, send }: HandleProps) {
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
function SegmentHandles({ ends, route, setDraft, send }: SegmentProps) {
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

interface Props {
  store: WireStore;
  /** Called with false when WebGL2 cannot draw the wires, and with true when it can again. */
  onReady: (ok: boolean) => void;
}

export function WireLayer({ store, onReady }: Props) {
  const ref = useRef<HTMLCanvasElement>(null);
  const flow = useStoreApi();

  useLayoutEffect(() => {
    const canvas = ref.current!;
    const host = canvas.parentElement!;
    let gl: GlWires;
    try {
      gl = new GlWires(canvas, store);
    } catch (e) {
      console.warn('The wires use SVG:', e);
      onReady(false);
      return;
    }
    let raf = 0;
    let lost = false;
    let ok = true;
    let zoomSet = NaN;
    const report = (now: boolean) => {
      if (now !== ok) onReady((ok = now));
    };
    const draw = () => {
      raf = 0;
      const [tx, ty, zoom] = flow.getState().transform;
      if (zoom !== zoomSet) {
        zoomSet = zoom;
        host.style.setProperty('--wire-hit', `${HIT_WIDTH / zoom}px`);
      }
      if (!lost) report(gl.draw(tx, ty, zoom));
    };
    const request = () => {
      if (raf === 0) raf = requestAnimationFrame(draw);
    };
    store.listen(request);
    const stop = flow.subscribe((s, prev) => {
      if (s.transform !== prev.transform) request();
    });
    const observer = new ResizeObserver(([entry]) => {
      const css = entry.contentRect;
      const device = entry.devicePixelContentBoxSize?.[0];
      const dpr = window.devicePixelRatio;
      gl.resize(css.width, css.height, device?.inlineSize ?? Math.round(css.width * dpr), device?.blockSize ?? Math.round(css.height * dpr));
      request();
    });
    observer.observe(canvas, { box: 'device-pixel-content-box' });
    const onLost = (e: Event) => {
      e.preventDefault();
      lost = true;
      report(false);
    };
    const onRestored = () => {
      lost = false;
      gl.init();
      report(true);
      request();
    };
    canvas.addEventListener('webglcontextlost', onLost);
    canvas.addEventListener('webglcontextrestored', onRestored);
    request();
    return () => {
      cancelAnimationFrame(raf);
      store.listen(() => {});
      stop();
      observer.disconnect();
      canvas.removeEventListener('webglcontextlost', onLost);
      canvas.removeEventListener('webglcontextrestored', onRestored);
      if (!lost) gl.dispose();
    };
  }, [store, flow, onReady]);

  return <canvas ref={ref} className="wire-layer" aria-hidden="true" />;
}

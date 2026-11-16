// The connections on the canvas (spec 15.1, 15.9). WebGL draws the visible wires under the nodes.
// Each React Flow edge keeps an invisible path on the same curve for the click, the box selection
// and the keyboard focus. Without WebGL2, CSS shows the paths of the edges instead.

import {
  createContext,
  useContext,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
  type KeyboardEvent,
  type MouseEvent,
  type PointerEvent,
} from 'react';
import { EdgeLabelRenderer, useReactFlow, useStore, useStoreApi, ViewportPortal, type Edge, type EdgeProps } from '@xyflow/react';
import type { Corners, Edit, KindInfo, Point, Style, Value } from './api';
import { HoverContext, PITCH, familyOf, hoveredLane, kindOf, laneAt, laneTip, wireBits } from './bus';
import { FAR_ZOOM, GlWires } from './gl';
import type { Router } from './route';
import { insertAt, midpoint, shape, wirePath, type WireStore } from './wires';

/** The hit area of a wire, in screen pixels, at each zoom. */
const HIT_WIDTH = 12;
/** The size of a route handle, in screen pixels. */
const HANDLE = 10;

/** The ends of a wire, its route points, its look, the type of its value and its bus name (spec 15.9). */
export type WireData = { from: string; to: string; route: readonly Point[]; style?: Style; corners?: Corners; ty?: string; fixed?: boolean; bus?: string };

export interface WireTools {
  store: WireStore;
  /** The automatic routes of the wires with the graph default look (15.9). */
  router: Router;
  onEdit: (edit: Edit) => Promise<void>;
  editable: boolean;
  /** Each live value by output (15.9), and true when it is from an old run. */
  values: ReadonlyMap<string, Value>;
  stale: (from: string) => boolean;
  /** The kinds of the connection types, for the lanes of each bus (6.3). */
  kinds: readonly KindInfo[];
}

export const WireContext = createContext<WireTools | null>(null);

export function WireEdge({ id, sourceX, sourceY, targetX, targetY, selected, data }: EdgeProps<Edge<WireData>>) {
  const { store, router, onEdit, editable, values, stale, kinds } = useContext(WireContext)!;
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
  const add = (e: MouseEvent) => {
    if (!editable || data!.fixed) return;
    const p = flow.screenToFlowPosition({ x: e.clientX, y: e.clientY });
    const k = insertAt(sourceX, sourceY, targetX, targetY, route, p.x, p.y);
    void send([...route.slice(0, k), [Math.round(p.x), Math.round(p.y)], ...route.slice(k)]);
  };
  const d = wirePath(sourceX, sourceY, targetX, targetY, path);
  const [mx, my] = midpoint(sourceX, sourceY, targetX, targetY, path);
  const n = bits.length;
  const ribbon = n > 1;
  // At a far zoom, only a selected wire shows its badge and type label. A bus keeps its width (15.1, 15.9).
  const near = !far || selected;
  // A bus shows its name and width in place of the type label, above its lanes.
  const bus = data!.bus ?? kind?.label ?? value?.lanes?.label ?? data!.ty ?? 'bus';
  const label = ribbon ? (far ? `/ ${n}` : `${bus}[${n - 1}:0] / ${n}`) : near ? data!.ty : undefined;
  const above = 12 + (ribbon && !far ? ((n - 1) * PITCH) / 2 : 0);
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
      <path className="wire-hit" d={d} style={hit} onDoubleClick={add} onPointerMove={move} onPointerLeave={leave} />
      <EdgeLabelRenderer>
        {tip && (
          <div className="lane-tip" style={{ transform: `translate(${tip.x}px, ${tip.y}px) translate(12px, calc(-100% - 8px))` }}>
            {laneTip(bus, kind?.lanes ?? [], bits, tip.lane, value !== undefined)}
          </div>
        )}
        {/* The type label or the bus label is above the badge (15.9). */}
        {label && (
          <div
            className={selected ? 'wire-type selected' : 'wire-type'}
            title={data!.ty}
            style={{ transform: `translate(${mx}px, ${my}px) translate(-50%, calc(-100% - ${above}px))` }}
          >
            {label}
          </div>
        )}
        {value !== undefined && near && (
          <div
            className={`wire-value${old ? ' stale' : ''}${selected ? ' selected' : ''}`}
            style={{ transform: `translate(${mx}px, ${my}px) translate(-50%, -50%)` }}
          >
            {value.text}
          </div>
        )}
      </EdgeLabelRenderer>
      {selected && editable && <RouteHandles route={route} dragging={draft !== null} setDraft={setDraft} send={send} />}
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

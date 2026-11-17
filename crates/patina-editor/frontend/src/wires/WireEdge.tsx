// The React Flow edges of the connections (spec 15.1, 15.9). Each edge keeps an invisible path on
// the same curve as its WebGL wire, for the click, the box selection and the keyboard focus.
// Without WebGL2, CSS shows the paths of the edges instead.

import { useContext, useEffect, useLayoutEffect, useMemo, useRef, useState, useSyncExternalStore, type MouseEvent, type PointerEvent } from 'react';
import { createPortal } from 'react-dom';
import { getBezierPath, Position, useReactFlow, useStore, type Edge, type EdgeProps, type ReactFlowState } from '@xyflow/react';
import type { Point } from '../api';
import { dragRoute } from '../canvas/frames';
import { HoverContext, PITCH, familyOf, hoveredLane, kindOf, laneAt, laneTip, wireBits, wireWidth } from '../canvas/bus';
import { FAR_ZOOM } from './gl';
import { RouteHandles, SegmentHandles } from './RouteHandles';
import type { WireIn } from './route';
import type { Router } from './router';
import { anchor, insertAt, shape, wirePath } from './wires';
import { WireContext, type WireData } from './WireLayer';

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

const labelHosts = new WeakMap<Element, Element>();

/** The label layer of the flow. EdgeLabelRenderer queries the DOM for it in each edge at each store change. */
function labelHost(s: ReactFlowState): Element | null {
  const dom = s.domNode;
  if (!dom) return null;
  let host = labelHosts.get(dom);
  if (!host?.isConnected) {
    host = dom.querySelector('.react-flow__edgelabel-renderer') ?? undefined;
    if (host) labelHosts.set(dom, host);
  }
  return host ?? null;
}

export function WireEdge({ id, sourceX, sourceY, targetX, targetY, selected, data }: EdgeProps<Edge<WireData>>) {
  const { store, router, labels, onEdit, editable, values, stale, kinds } = useContext(WireContext)!;
  const hover = useContext(HoverContext);
  const flow = useReactFlow();
  // The route during a drag. It stays until the edit gives the new graph, so the wire does not jump back.
  const [draft, setDraft] = useState<readonly Point[] | null>(null);
  const base = draft ?? data!.route;
  // The route points of an inner wire move with its use during a drag (15.3).
  const frame = data!.frame;
  const now = useStore((s) => (frame ? s.nodeLookup.get(frame.use)?.position : undefined));
  const route = useMemo(() => dragRoute(base, frame, now), [base, frame, now]);
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
  const layer = useStore(labelHost);
  const { from, style, corners, cross = false, net = from } = data!;
  // A wire with the graph default look goes around the node bodies. Its route points are waypoints (15.9).
  const auto = style === undefined;
  const width = wireWidth(bits);
  const wire = auto ? { sx: sourceX, sy: sourceY, tx: targetX, ty: targetY, via: route, net, width } : null;
  const { routed, conflict, joints, gaps } = useAutoRoute(router, id, wire);
  // The style of the drawn path. An automatic wire is orthogonal until the router gives its bends.
  const drawn = auto ? (routed ? 'polyline' : 'orthogonal') : style;
  const path = useMemo(
    () => shape(sourceX, sourceY, targetX, targetY, (auto && routed) || route, drawn, corners, width),
    [auto, sourceX, sourceY, targetX, targetY, routed, route, drawn, corners, width],
  );
  useLayoutEffect(
    () => store.set(id, sourceX, sourceY, targetX, targetY, path, !!selected, bits, old, family, lane, cross, gaps),
    [store, id, sourceX, sourceY, targetX, targetY, path, selected, bits, old, family, lane, cross, gaps],
  );
  useLayoutEffect(() => () => store.remove(id), [store, id]);
  const send = (points: Point[]) => onEdit({ kind: 'SetRoute', to: data!.to, points });
  const insert = (x: number, y: number) => {
    if (!editable || data!.fixed) return;
    const k = insertAt(sourceX, sourceY, targetX, targetY, route, x, y, drawn);
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
      {/* shortcut: the SVG fallback has no crossing gaps (15.9); add a mask when the fallback is more than a lost context. */}
      <path className={cross ? 'wire-line cross' : 'wire-line'} d={d} />
      <path ref={hitRef} className="wire-hit" d={d} style={hit} onDoubleClick={add} onPointerMove={move} onPointerLeave={leave} />
      {layer && createPortal(
        <>
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
          {/* A dot where the wires of one output part, a ring on a bus (15.9). */}
          {joints?.map(([x, y]) => (
            <div
              key={`${x} ${y}`}
              className={width > 2 ? 'wire-joint bus' : 'wire-joint'}
              aria-hidden
              style={{ transform: `translate(${x}px, ${y}px) translate(-50%, -50%)`, ...(width > 2 && { width: width + 6, height: width + 6 }) }}
            />
          ))}
          {badge !== undefined && !badgeCovered && (
            <div
              className={`wire-value${old ? ' stale' : ''}${selected ? ' selected' : ''}`}
              style={{ transform: `translate(${mx}px, ${my}px) translate(-50%, -50%)`, maxWidth: badgeRoom }}
            >
              {badge}
            </div>
          )}
        </>,
        layer,
      )}
      {selected && editable && <RouteHandles route={route} dragging={draft !== null} setDraft={setDraft} send={send} />}
      {selected && editable && style === 'orthogonal' && !data!.fixed && (
        <SegmentHandles ends={[sourceX, sourceY, targetX, targetY]} route={route} setDraft={setDraft} send={send} />
      )}
    </>
  );
}

const NO_WIRE: WireIn = { sx: 0, sy: 0, tx: 0, ty: 0, via: [], net: '', width: 0 };

/**
 * Gives a wire with the graph default look to the router (15.9). A wire with its own look is null.
 * It returns the bends, the conflict mark, the joints and the crossing gaps of the wire.
 */
function useAutoRoute(router: Router, id: string, wire: WireIn | null) {
  const auto = wire !== null;
  const { sx, sy, tx, ty, via, net, width } = wire ?? NO_WIRE;
  useLayoutEffect(() => {
    if (auto) router.setWire(id, { sx, sy, tx, ty, via, net, width });
    else router.removeWire(id);
  }, [router, id, auto, sx, sy, tx, ty, via, net, width]);
  useLayoutEffect(() => () => router.removeWire(id), [router, id]);
  const routed = useSyncExternalStore(router.subscribe, () => (auto ? router.bends(id) : undefined));
  const conflict = useSyncExternalStore(router.subscribe, () => (auto ? router.conflict(id) : undefined));
  const joints = useSyncExternalStore(router.subscribe, () => (auto ? router.junctions(id) : undefined));
  const gaps = useSyncExternalStore(router.subscribe, () => (auto ? router.gaps(id) : undefined));
  return { routed, conflict, joints, gaps };
}

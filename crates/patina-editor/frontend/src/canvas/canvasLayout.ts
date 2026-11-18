// The layout state of the canvas: the nodes in view, the open frames and the route obstacles.
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { useStore } from '@xyflow/react';
import { api, type GraphFile, type View } from '../api';
import { gutter, HEADER, ROW } from './NodeView';
import { FAR_ZOOM } from '../wires/gl';
import { NODE_WIDTH } from '../controls/gui';
import { frameOf, shownFrames, toggleFrame, type Room } from './frames';
import type { Labels } from '../wires/labels';
import type { Rect } from '../wires/route';
import type { Router } from '../wires/router';
import type { GraphNode } from './graphData';

/** Tells the engine the nodes in view, so that the frames draw only their GUIs (4.3 rule 3). None below FAR_ZOOM. */
export function useVisibleGuis() {
  const visible = useStore((s) => {
    const [tx, ty, zoom] = s.transform;
    if (zoom < FAR_ZOOM) return '';
    const [x0, y0] = [-tx / zoom, -ty / zoom];
    const [x1, y1] = [x0 + s.width / zoom, y0 + s.height / zoom];
    const ids: string[] = [];
    for (const n of s.nodeLookup.values()) {
      const { x, y } = n.internals.positionAbsolute;
      const [w, h] = [n.measured.width ?? 0, n.measured.height ?? 0];
      if (n.type === 'patina' && x <= x1 && x + w >= x0 && y <= y1 && y + h >= y0) ids.push(n.id);
    }
    return ids.join('\n');
  });
  useEffect(() => {
    const t = setTimeout(() => api.visible(visible === '' ? [] : visible.split('\n')).catch(() => {}), 50);
    return () => clearTimeout(t);
  }, [visible]);
}

/** Gives the node bodies to the automatic routes and the wire names, which go around them (15.9). */
export function useObstacles(nodes: GraphNode[], router: Router, labels: Labels) {
  useLayoutEffect(() => {
    const at = new Map(nodes.map((n) => [n.id, n.position]));
    const body = (n: GraphNode): Rect[] => {
      const { width, height } = n.measured ?? {};
      if (!width || !height) return [];
      const p = n.parentId ? at.get(n.parentId) : undefined;
      const [l, t] = [n.position.x + (p?.x ?? 0) + gutter(n), n.position.y + (p?.y ?? 0)];
      const [r, b] = [l + width - 2 * gutter(n), t + height];
      if (n.type !== 'patina' || !n.data.open) return [{ l, t, r, b }];
      // A frame blocks at its header and its thin edges, so the inner wires route inside it. The wires
      // at the frame pins cross the side edges within the pin margin of the router.
      return [{ l, t, r, b: t + HEADER }, { l: l - 1, t: t + HEADER, r: l, b }, { l: r, t: t + HEADER, r: r + 1, b }, { l, t: b - 1, r, b }];
    };
    const rects = nodes.flatMap(body);
    router.setObstacles(rects);
    labels.setObstacles(rects);
  }, [nodes, router, labels]);
}

/** The uses that show their inner nodes (15.3): their frames, the room of each frame and the toggle. */
export function useFrames(view: View, nodes: GraphNode[]) {
  const [shown, setShown] = useState(() => shownFrames(view.graphName));
  const [files, setFiles] = useState<ReadonlyMap<string, GraphFile>>(() => new Map());
  useEffect(() => {
    const names = [...new Set((view.graph.node ?? []).flatMap((n) => (shown.has(n.id) && n.subgraph !== undefined ? [n.subgraph] : [])))];
    let live = true;
    // A subgraph that does not read shows no frame.
    Promise.all(names.map((name) => api.subgraphFile(name).then((file) => [name, file] as const, () => null))).then((list) => {
      if (live) setFiles((prev) => (list.length === 0 && prev.size === 0 ? prev : new Map(list.filter((x) => x !== null))));
    });
    return () => {
      live = false;
    };
  }, [view, shown]);
  const frames = useMemo(
    () =>
      (view.graph.node ?? []).flatMap((n) => {
        const file = shown.has(n.id) && n.subgraph !== undefined ? files.get(n.subgraph) : undefined;
        return file ? [frameOf(n.id, file)] : [];
      }),
    [view, files, shown],
  );
  const toggle = useCallback((use: string) => setShown(toggleFrame(view.graphName, use)), [view.graphName]);

  // The room of each open frame, from its measured size and the last closed size of its use (15.3).
  const closed = useRef(new Map<string, readonly [number, number]>());
  const [rooms, setRooms] = useState<readonly Room[]>([]);
  useLayoutEffect(() => {
    const byId = new Map(nodes.map((n) => [n.id, n]));
    for (const n of nodes) {
      if (n.type === 'patina' && !n.data.open && n.measured?.width && n.measured.height) closed.current.set(n.id, [n.measured.width - 2 * gutter(n), n.measured.height]);
    }
    const next = frames.flatMap((f): Room[] => {
      const n = byId.get(f.use);
      if (n?.type !== 'patina' || !n.data.open || !n.measured?.width || !n.measured.height) return [];
      // A use that was not closed in this view gets the smallest closed size, so its frame never covers a node.
      const rows = Math.max(n.data.info?.inputs.length ?? 0, n.data.info?.outputs.length ?? 0);
      const [w, h] = closed.current.get(f.use) ?? [NODE_WIDTH, HEADER + rows * ROW];
      const [x, y] = n.data.entry.pos ?? [0, 0];
      return [{ use: f.use, x, y, w, h, dx: n.measured.width - 2 * gutter(n) - w, dy: n.measured.height - h }];
    });
    setRooms((prev) => (JSON.stringify(prev) === JSON.stringify(next) ? prev : next));
  }, [nodes, frames]);
  return { frames, rooms, toggle };
}

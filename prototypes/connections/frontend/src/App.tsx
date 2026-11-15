// The graph view: React Flow for the nodes, and the SVG or the WebGL renderer for the connections.

import { useEffect, useLayoutEffect, useRef, useState, useSyncExternalStore } from 'react';
import {
  Background,
  BackgroundVariant,
  Panel,
  ReactFlow,
  ReactFlowProvider,
  ViewportPortal,
  useReactFlow,
  useStoreApi,
  type EdgeTypes,
  type NodeTypes,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import { invoke } from '@tauri-apps/api/core';
import { Bench, type BenchHost } from './bench';
import { GlWires, gpuName } from './gl';
import { Lanes, startFeed } from './lanes';
import { NodeView, type PatinaNodeType } from './NodeView';
import { LaneEdge, SvgContext, SvgLive, type LaneEdgeType } from './svg';
import type { Config, Graph } from './types';
import { NEAR_ZOOM, Wires } from './wires';

const nodeTypes: NodeTypes = { patina: NodeView };
const edgeTypes: EdgeTypes = { lane: LaneEdge };

export function reportError(e: unknown) {
  const message = e instanceof Error ? `${e.message}\n${e.stack ?? ''}` : String(e);
  console.error(message);
  invoke('page_error', { message }).catch(() => {});
}

/** The selected edge. Its version changes when the selection or the route of the selection changes. */
class Selection {
  edge = -1;
  version = 0;
  private readonly listeners = new Set<() => void>();
  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => void this.listeners.delete(listener);
  };
  snapshot = () => this.version;
  set(edge: number) {
    this.edge = edge;
    this.bump();
  }
  bump() {
    this.version++;
    for (const l of this.listeners) l();
  }
}

interface Model {
  wires: Wires;
  lanes: Lanes;
  svg: SvgLive | null;
  gl: GlWires | null;
  selection: Selection;
  rev: number;
}

function RouteHandles({ model, move }: { model: Model; move: (i: number, k: number, x: number, y: number) => void }) {
  useSyncExternalStore(model.selection.subscribe, model.selection.snapshot);
  const { screenToFlowPosition } = useReactFlow();
  const i = model.selection.edge;
  if (i < 0) return null;
  return (
    <ViewportPortal>
      {model.wires.graph.edges[i].route.map(([x, y], k) => (
        <div
          key={k}
          className="route-point nodrag nopan"
          style={{ transform: `translate(${x}px, ${y}px) translate(-50%, -50%)` }}
          onPointerDown={(e) => {
            e.stopPropagation();
            e.currentTarget.setPointerCapture(e.pointerId);
          }}
          onPointerMove={(e) => {
            if (!e.currentTarget.hasPointerCapture(e.pointerId)) return;
            const p = screenToFlowPosition({ x: e.clientX, y: e.clientY });
            move(i, k, p.x, p.y);
          }}
        />
      ))}
    </ViewportPortal>
  );
}

function GraphView({ config, graph }: { config: Config; graph: Graph }) {
  const rf = useReactFlow<PatinaNodeType, LaneEdgeType>();
  const store = useStoreApi();
  const glRef = useRef<HTMLCanvasElement>(null);
  const overlayRef = useRef<HTMLCanvasElement>(null);
  const hudRef = useRef<HTMLDivElement>(null);
  const webgl = config.renderer === 'webgl';

  const [model] = useState<Model>(() => {
    const wires = new Wires(graph);
    const lanes = new Lanes(graph.laneCount);
    const svg = webgl ? null : new SvgLive(wires, lanes);
    return { wires, lanes, svg, gl: null, selection: new Selection(), rev: 0 };
  });

  const [defaults] = useState(() => {
    const nodes: PatinaNodeType[] = graph.nodes.map((n, k) => ({
      id: `n${k}`,
      type: 'patina',
      position: { x: n.x, y: n.y },
      width: n.w,
      height: n.h,
      data: { node: n, pitch: graph.lanePitch },
      draggable: false,
      selectable: false,
      focusable: false,
    }));
    const edges: LaneEdgeType[] = webgl
      ? []
      : graph.edges.map((e, i) => ({
          id: `e${i}`,
          type: 'lane',
          source: `n${e.source}`,
          sourceHandle: `o${e.sourcePin}`,
          target: `n${e.target}`,
          targetHandle: `i${e.targetPin}`,
          data: { index: i, rev: 0 },
          focusable: false,
        }));
    return { nodes, edges };
  });

  const select = (i: number) => {
    model.selection.set(i);
    if (model.gl) {
      model.gl.setSelected(i);
    } else if (i < 0) {
      store.getState().unselectNodesAndEdges();
    } else {
      store.getState().addSelectedEdges([`e${i}`]);
    }
  };

  const move = (i: number, k: number, x: number, y: number) => {
    const old = model.wires.counts[i];
    model.wires.moveRoutePoint(i, k, x, y);
    if (model.gl) model.gl.updateEdge(i, old);
    else rf.updateEdgeData(`e${i}`, { rev: ++model.rev });
    model.selection.bump();
  };

  useLayoutEffect(() => {
    if (!webgl) return;
    const canvas = glRef.current!;
    const gl = new GlWires(canvas, overlayRef.current!, model.wires, model.lanes);
    model.gl = gl;
    const observer = new ResizeObserver(([entry]) => {
      const css = entry.contentRect;
      const device = entry.devicePixelContentBoxSize?.[0];
      const dpr = device && css.width > 0 ? device.inlineSize / css.width : window.devicePixelRatio;
      gl.resize(css.width, css.height, dpr);
    });
    observer.observe(canvas, { box: 'device-pixel-content-box' });
    return () => observer.disconnect();
  }, [webgl, model]);

  useEffect(() => {
    const host: BenchHost = {
      setView: (x, y, zoom) => void rf.setViewport({ x, y, zoom }, { duration: 0 }),
      fit: () => void rf.fitView({ duration: 0 }),
      zoom: () => store.getState().transform[2],
      select,
      moveRoute: move,
      gpu: () => model.gl?.renderer() ?? probeGpu(),
    };
    const bench = config.bench ? new Bench(config, model.wires, model.lanes, host, reportError) : null;
    startFeed(config.feed, model.lanes, reportError).catch(reportError);

    const port = new MessageChannel();
    let raf = 0;
    let prev = 0;
    let frameStart = 0;
    let interval = 0;
    let seen = -1;
    let hudFrames = 0;
    let hudTime = 0;
    let hudWork = 0;
    // The message runs after the rendering steps of the frame, so `work` includes React, style, layout and paint.
    port.port1.onmessage = () => {
      const work = performance.now() - frameStart;
      bench?.record(interval, work);
      if (!hudRef.current || interval <= 0) return;
      hudFrames++;
      hudTime += interval;
      hudWork = Math.max(hudWork, work);
      if (hudFrames < 30) return;
      const zoom = store.getState().transform[2];
      hudRef.current.textContent = `${config.renderer}/${config.feed}  ${((hudFrames * 1000) / hudTime).toFixed(1)} fps  work max ${hudWork.toFixed(1)} ms  zoom ${(zoom * 100).toFixed(0)}%  dpr ${window.devicePixelRatio}`;
      hudFrames = 0;
      hudTime = 0;
      hudWork = 0;
    };
    const loop = (ts: number) => {
      raf = requestAnimationFrame(loop);
      frameStart = performance.now();
      interval = prev > 0 ? ts - prev : 0;
      prev = ts;
      try {
        bench?.before(ts);
        if (model.svg && model.lanes.version !== seen) {
          seen = model.lanes.version;
          model.svg.apply();
        }
        const [tx, ty, zoom] = store.getState().transform;
        model.gl?.draw(tx, ty, zoom);
      } catch (e) {
        cancelAnimationFrame(raf);
        reportError(e);
      }
      port.port2.postMessage(0);
    };
    raf = requestAnimationFrame(loop);

    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'f' || e.key === 'F') rf.fitView({ duration: 0 });
      if (e.key === 'Escape') select(-1);
    };
    if (!config.bench) window.addEventListener('keydown', onKey);
    return () => {
      cancelAnimationFrame(raf);
      port.port1.close();
      window.removeEventListener('keydown', onKey);
    };
    // The loop reads the model, the store and the instance, which do not change.
  }, []);

  return (
    <SvgContext.Provider value={model.svg}>
      <ReactFlow
        defaultNodes={defaults.nodes}
        defaultEdges={defaults.edges}
        nodeTypes={nodeTypes}
        edgeTypes={edgeTypes}
        onlyRenderVisibleElements
        panOnDrag={[1]}
        selectionOnDrag
        zoomOnDoubleClick={false}
        deleteKeyCode={null}
        nodesConnectable={false}
        minZoom={0.1}
        maxZoom={4}
        fitView
        onEdgeClick={(_e, edge) => select(edge.data!.index)}
        onPaneClick={(e) => {
          if (!model.gl) return select(-1);
          const p = rf.screenToFlowPosition({ x: e.clientX, y: e.clientY });
          const zoom = store.getState().transform[2];
          select(model.wires.hit(p.x, p.y, 4 / zoom, zoom >= NEAR_ZOOM));
        }}
      >
        <Background id="minor" variant={BackgroundVariant.Lines} gap={24} color="#202020" />
        <Background id="major" variant={BackgroundVariant.Lines} gap={120} color="#292929" />
        {webgl && <canvas ref={glRef} className="gl-layer" />}
        {webgl && <canvas ref={overlayRef} className="gl-layer" />}
        <RouteHandles model={model} move={move} />
        {!config.bench && (
          <Panel position="top-left">
            <div ref={hudRef} className="hud" />
          </Panel>
        )}
      </ReactFlow>
    </SvgContext.Provider>
  );
}

/** The GPU name for the SVG runs, which have no WebGL context. */
function probeGpu(): string {
  const gl = document.createElement('canvas').getContext('webgl2');
  if (!gl) return 'unknown';
  const name = gpuName(gl);
  gl.getExtension('WEBGL_lose_context')?.loseContext();
  return name;
}

export function App({ config, graph }: { config: Config; graph: Graph }) {
  return (
    <ReactFlowProvider>
      <GraphView config={config} graph={graph} />
    </ReactFlowProvider>
  );
}

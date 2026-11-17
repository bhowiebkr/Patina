// The WebGL layer of the connections on the canvas (spec 15.1, 15.9). WebGL draws the visible
// wires under the nodes. The React Flow edges are in WireEdge.tsx.

import { createContext, useLayoutEffect, useRef } from 'react';
import { useStoreApi } from '@xyflow/react';
import type { Corners, Edit, KindInfo, Path, Point, Value } from '../api';
import type { FrameAt } from '../canvas/frames';
import { GlWires } from './gl';
import type { Labels } from './labels';
import type { Router } from './router';
import type { WireStore } from './wires';

/** The hit area of a wire, in screen pixels, at each zoom. */
const HIT_WIDTH = 12;

/** The ends of a wire, its route points, its look, the type of its value, its wire name and its bus name (spec 15.9). */
/** `cross` is true for a wire between two clock domains, which is dashed (15.3). `net` is the route net when it is not `from`. */
/** `frame` is the use of an inner wire of a frame. */
export type WireData = { from: string; to: string; route: readonly Point[]; frame?: FrameAt; style?: Path; corners?: Corners; ty?: string; fixed?: boolean; name?: string; bus?: string; cross?: boolean; net?: string };

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

// The connections on the canvas (spec 15.1, 15.9). WebGL draws the visible wires under the nodes.
// Each React Flow edge keeps an invisible path on the same curve for the click, the box selection
// and the keyboard focus. Without WebGL2, CSS shows the paths of the edges instead.

import { createContext, useContext, useLayoutEffect, useRef } from 'react';
import { useStoreApi, type EdgeProps } from '@xyflow/react';
import { GlWires } from './gl';
import { wirePath, type WireStore } from './wires';

/** The hit area of a wire, in screen pixels, at each zoom. */
const HIT_WIDTH = 12;

export const WireContext = createContext<WireStore | null>(null);

export function WireEdge({ id, sourceX, sourceY, targetX, targetY, selected }: EdgeProps) {
  const store = useContext(WireContext)!;
  useLayoutEffect(() => store.set(id, sourceX, sourceY, targetX, targetY, !!selected), [store, id, sourceX, sourceY, targetX, targetY, selected]);
  useLayoutEffect(() => () => store.remove(id), [store, id]);
  const d = wirePath(sourceX, sourceY, targetX, targetY);
  return (
    <>
      <path className="wire-line" d={d} />
      <path className="wire-hit" d={d} />
    </>
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

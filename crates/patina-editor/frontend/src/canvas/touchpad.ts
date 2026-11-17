import { useEffect, useRef, type RefObject } from 'react';
import type { Viewport } from '@xyflow/react';

/** The wheel fields that tell a touchpad from a mouse wheel. `wheelDeltaX/Y` are Chromium fields. */
export interface Wheel {
  deltaMode: number;
  ctrlKey: boolean;
  wheelDeltaX?: number;
  wheelDeltaY?: number;
}

/** In this time in ms, the next wheel event continues a touchpad scroll. */
const GESTURE_MS = 150;

/**
 * Tells if a wheel event is a two-finger touchpad scroll, which pans the graph (15.2). A mouse
 * wheel step is a multiple of 120 in `wheelDelta`, a touchpad gives other values. A pinch has
 * `ctrlKey` and zooms. `since` is the time in ms since the last touchpad scroll event.
 */
export function isTouchpadScroll(e: Wheel, since: number): boolean {
  if (e.ctrlKey || e.deltaMode !== 0) return false;
  if ((e.wheelDeltaX ?? 0) % 120 !== 0 || (e.wheelDeltaY ?? 0) % 120 !== 0) return true;
  return since < GESTURE_MS;
}

interface Flow {
  getViewport(): Viewport;
  setViewport(v: Viewport): Promise<boolean>;
}

/**
 * Pans the view in `pane` with a two-finger touchpad scroll on both axes (15.2). React Flow keeps
 * the wheel zoom and the pinch. `save` gets the view at the end of a scroll. The returned ref is
 * true during a scroll.
 */
export function useTouchpadPan(pane: RefObject<HTMLElement | null>, flow: Flow, save: (v: Viewport) => void): RefObject<boolean> {
  const scrolling = useRef(false);
  useEffect(() => {
    const el = pane.current;
    if (!el) return;
    let last = -Infinity;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const onWheel = (e: WheelEvent) => {
      if (e.target instanceof Element && e.target.closest('.nowheel')) return;
      if (!isTouchpadScroll(e, e.timeStamp - last)) return;
      last = e.timeStamp;
      e.preventDefault();
      e.stopPropagation();
      const { x, y, zoom } = flow.getViewport();
      void flow.setViewport({ x: x - e.deltaX, y: y - e.deltaY, zoom });
      // One save at the end of the scroll, not one for each event.
      scrolling.current = true;
      clearTimeout(timer);
      timer = setTimeout(() => {
        scrolling.current = false;
        save(flow.getViewport());
      }, GESTURE_MS);
    };
    el.addEventListener('wheel', onWheel, { capture: true, passive: false });
    return () => {
      el.removeEventListener('wheel', onWheel, { capture: true });
      clearTimeout(timer);
      scrolling.current = false;
    };
  }, [pane, flow, save]);
  return scrolling;
}

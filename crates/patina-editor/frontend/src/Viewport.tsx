// The viewport widget (spec 15.8). The view draws into the window under the page, so the page is
// transparent over the area. Each frame sends the place of the area. The pointer, the wheel and the
// keys go to the view too. The sizes are in physical pixels.

import { useEffect, useRef, type KeyboardEvent, type PointerEvent } from 'react';
import type { Widget } from './gui';

/** The class on the page root that makes the parents of a viewport transparent (canvas.css). */
const CLASS = 'native-viewport';
/** The frames before the page goes transparent, so that the view draws first. */
const SHOW_AFTER = 3;

type Props = { w: Extract<Widget, { kind: 'viewport' }>; send: (value: string) => void; disabled: boolean; window: number };

export function ViewportView({ w, send, disabled, window: handle }: Props) {
  const ref = useRef<HTMLDivElement>(null);
  const out = useRef(send);
  out.current = send;
  const held = useRef(new Set<string>());

  useEffect(() => {
    const area = ref.current;
    if (!handle || !area) return;
    const px = (v: number) => Math.max(0, Math.round(v * devicePixelRatio));
    let frame = 0;
    let count = 0;
    const tick = () => {
      const r = area.getBoundingClientRect();
      // A scroll of the panel clips the area, so the scene does not show outside the panel.
      const c = area.closest('.panel')?.getBoundingClientRect() ?? r;
      const [left, top] = [Math.max(r.left, c.left), Math.max(r.top, c.top)];
      const [right, bottom] = [Math.min(r.right, c.right), Math.min(r.bottom, c.bottom)];
      const size = `${px(Math.max(0, right - left))} ${px(Math.max(0, bottom - top))}`;
      out.current(`place ${handle} ${px(innerWidth)} ${px(innerHeight)} ${px(left)} ${px(top)} ${size}`);
      if (++count === SHOW_AFTER) document.documentElement.classList.add(CLASS);
      frame = requestAnimationFrame(tick);
    };
    // A native listener, because the wheel must not scroll the panel.
    const wheel = (e: WheelEvent) => {
      e.preventDefault();
      if (!disabled) out.current(`wheel ${Math.round(e.deltaY * 100) / 100}`);
    };
    area.addEventListener('wheel', wheel, { passive: false });
    tick();
    return () => {
      cancelAnimationFrame(frame);
      area.removeEventListener('wheel', wheel);
      document.documentElement.classList.remove(CLASS);
    };
  }, [handle, disabled]);

  if (!handle) return <div className="item empty">{w.text || 'Viewport'}: a viewport shows only in a view, on Windows.</div>;
  const at = (e: PointerEvent<HTMLDivElement>) => {
    const r = e.currentTarget.getBoundingClientRect();
    return `${Math.round((e.clientX - r.left) * devicePixelRatio)} ${Math.round((e.clientY - r.top) * devicePixelRatio)}`;
  };
  const drag = (e: PointerEvent<HTMLDivElement>) => e.currentTarget.hasPointerCapture(e.pointerId);
  // Tab keeps its job, so the focus can leave the viewport.
  const key = (down: boolean) => (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key === 'Tab' || (down && e.repeat)) return;
    e.preventDefault();
    if (down === held.current.has(e.code)) return;
    if (down) held.current.add(e.code);
    else held.current.delete(e.code);
    send(`key ${down ? 'down' : 'up'} ${e.code}`);
  };
  const release = () => {
    for (const code of held.current) send(`key up ${code}`);
    held.current.clear();
  };
  return (
    <div className="gui-canvas-row">
      {w.text && <span className="label">{w.text}</span>}
      <div
        ref={ref}
        className="gui-viewport"
        style={{ height: w.height }}
        role="application"
        aria-label={w.text || 'Viewport'}
        tabIndex={disabled ? -1 : 0}
        onKeyDown={disabled ? undefined : key(true)}
        onKeyUp={disabled ? undefined : key(false)}
        onBlur={release}
        onPointerDown={
          disabled
            ? undefined
            : (e) => {
                e.currentTarget.setPointerCapture(e.pointerId);
                e.currentTarget.focus();
                send(`down ${at(e)}`);
              }
        }
        onPointerMove={disabled ? undefined : (e) => drag(e) && send(`move ${at(e)}`)}
        onPointerUp={disabled ? undefined : (e) => drag(e) && send(`up ${at(e)}`)}
      />
    </div>
  );
}

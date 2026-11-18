// The 2D transform of a node GUI (spec 4.3): a gizmo on the image, and the number fields under it
// (the Transform node of mockup 4). A drag shows its own transform and sends it at the end.

import { useMemo, useRef, useState, type KeyboardEvent, type PointerEvent } from 'react';
import { Field } from './Field';
import { applyTransform, dragTransform, movePivot, scaleTo, transformEvent, type Part, type Point, type Transform } from './gui';
import { own, Titled, type Input } from './ValueWidgets';

// The display is 200 units wide and at most 160 units high, with a margin for a box out of the frame.
const W = 200;
const MAX_H = 160;
const PAD = 18;
const PIVOTS = ['Top left', 'Top', 'Top right', 'Left', 'Center', 'Right', 'Bottom left', 'Bottom', 'Bottom right'];

export function TransformGizmo({ w, send, disabled }: Input<'transform'>) {
  const [drag, setDrag] = useState<{ part: Part; from: Point; to: Point; snap: boolean } | null>(null);
  const svg = useRef<SVGSVGElement>(null);
  const [fw, fh] = w.frame;
  const k = Math.min((W - 2 * PAD) / fw, (MAX_H - 2 * PAD) / fh);
  const h = fh * k + 2 * PAD;
  const ox = (W - fw * k) / 2;
  const view = ([x, y]: Point): Point => [ox + x * k, PAD + y * k];
  const pivot = (t: Transform) => view([t.px * fw + t.x, t.py * fh + t.y]);
  const t = drag ? dragTransform(w.t, drag.part, drag.from, drag.to, pivot(w.t), k, drag.snap) : w.t;
  const set = (next: Transform) => send(transformEvent(next));
  const [iw, ih, rgba] = w.image ? [w.image.width, w.image.height, w.image.rgba] : [0, 0, ''];
  // The image goes into the SVG as a PNG. It changes only when its pixels change.
  const url = useMemo(() => {
    if (!rgba) return '';
    const canvas = document.createElement('canvas');
    [canvas.width, canvas.height] = [iw, ih];
    const bytes = Uint8ClampedArray.from(atob(rgba), (c) => c.charCodeAt(0));
    canvas.getContext('2d')?.putImageData(new ImageData(bytes, iw, ih), 0, 0);
    return canvas.toDataURL();
  }, [iw, ih, rgba]);
  const at = (e: PointerEvent): Point => {
    const r = svg.current!.getBoundingClientRect();
    return [((e.clientX - r.left) / r.width) * W, ((e.clientY - r.top) / r.height) * h];
  };
  const down = (e: PointerEvent<SVGSVGElement>) => {
    const part = (e.target as Element).closest('[data-part]')?.getAttribute('data-part') as Part | null;
    if (e.button !== 0 || !part) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    setDrag({ part, from: at(e), to: at(e), snap: e.shiftKey });
  };
  const move = (e: PointerEvent<SVGSVGElement>) => {
    if (drag) setDrag({ ...drag, to: at(e), snap: e.shiftKey });
  };
  const up = () => {
    if (drag && transformEvent(t) !== transformEvent(w.t)) set(t);
    setDrag(null);
  };
  const key = (e: KeyboardEvent) => {
    const step = e.shiftKey ? 10 : 1;
    const [dx, dy] = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step] }[e.key] ?? [0, 0];
    if (!dx && !dy) return;
    own(e);
    set({ ...w.t, x: w.t.x + dx, y: w.t.y + dy });
  };
  const corners = ([[0, 0], [fw, 0], [fw, fh], [0, fh]] as Point[]).map((c) => view(applyTransform(t, w.frame, c)));
  const [px, py] = pivot(t);
  const arrow = (dx: number, dy: number) => `M${px - dx} ${py - dy}L${px + dx} ${py + dy}`;
  const head = (dx: number, dy: number) => `M${px + dx + dy * 0.25 - dx * 0.25} ${py + dy - dx * 0.25 - dy * 0.25}L${px + dx} ${py + dy}L${px + dx - dy * 0.25 - dx * 0.25} ${py + dy + dx * 0.25 - dy * 0.25}`;
  const part = (name: Part, d: string) => (
    <g data-part={name} className={`gizmo-${name}`}>
      <path className="hit" d={d} />
      <path d={d} />
    </g>
  );
  const name = w.text || 'Transform';
  const number = (text: string, value: number, unit: string, apply: (v: number) => Transform) => (
    <Field w={{ kind: 'number', index: w.index, text, value, step: 0, unit }} send={(v) => set(apply(Number(v)))} disabled={disabled} />
  );
  const preset = PIVOTS.findIndex((_, i) => (i % 3) / 2 === w.t.px && Math.floor(i / 3) / 2 === w.t.py);
  return (
    <Titled text={w.text}>
      <svg
        ref={svg}
        className="gui-transform"
        viewBox={`0 0 ${W} ${h}`}
        role="group"
        aria-label={`${name}: drag the box to move, the ring to rotate and a corner to scale`}
        onPointerDown={disabled ? undefined : down}
        onPointerMove={disabled ? undefined : move}
        onPointerUp={disabled ? undefined : up}
        onPointerCancel={() => setDrag(null)}
        onLostPointerCapture={() => setDrag(null)}
      >
        {url && <image href={url} x={ox} y={PAD} width={fw * k} height={fh * k} preserveAspectRatio="none" />}
        <rect className="frame" x={ox} y={PAD} width={fw * k} height={fh * k} />
        <g className={drag ? 'gizmo dragging' : 'gizmo'}>
          <polygon
            data-part="move"
            className="box"
            points={corners.map((c) => c.join(',')).join(' ')}
            tabIndex={disabled ? -1 : 0}
            role="button"
            aria-label={`${name} box: the arrow keys move it`}
            onKeyDown={disabled ? undefined : key}
          />
          {part('rotate', `M${px + 24} ${py}A24 24 0 1 1 ${px + 24} ${py - 0.01}`)}
          {part('x', arrow(16, 0) + head(16, 0) + head(-16, 0))}
          {part('y', arrow(0, 16) + head(0, 16) + head(0, -16))}
          <circle data-part="move" className="pivot" cx={px} cy={py} r={3} />
          {corners.map(([x, y], i) => (
            <rect key={i} data-part="corner" className="corner" x={x - 3.5} y={y - 3.5} width={7} height={7} />
          ))}
        </g>
      </svg>
      <div className="gui-transform-fields">
        {number('X', t.x, 'px', (x) => ({ ...w.t, x }))}
        {number('Y', t.y, 'px', (y) => ({ ...w.t, y }))}
        {number('Rot', t.rotation, 'deg', (rotation) => ({ ...w.t, rotation }))}
        {w.t.lock ? (
          number('Scale', t.sx, '', (v) => scaleTo(w.t, v))
        ) : (
          <>
            {number('Scale X', t.sx, '', (v) => scaleTo(w.t, v))}
            {number('Scale Y', t.sy, '', (sy) => ({ ...w.t, sy }))}
          </>
        )}
      </div>
      <div className="gui-transform-fields">
        <span className="label">Pivot</span>
        <div className="gui-pivots" role="radiogroup" aria-label={`${name} pivot`}>
          {PIVOTS.map((p, i) => (
            <button
              key={p}
              className={i === preset ? 'gui-pivot selected' : 'gui-pivot'}
              role="radio"
              aria-checked={i === preset}
              aria-label={p}
              title={p}
              disabled={disabled}
              onClick={() => set(movePivot(w.t, w.frame, [(i % 3) / 2, Math.floor(i / 3) / 2]))}
            />
          ))}
        </div>
        <label className="gui-row">
          <input type="checkbox" checked={w.t.lock} disabled={disabled} onChange={(e) => set({ ...w.t, lock: e.currentTarget.checked })} />
          <span>Aspect lock</span>
        </label>
      </div>
    </Titled>
  );
}

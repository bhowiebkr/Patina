// The color ramp of a node GUI (spec 4.3): the gradient spans the width with the stops below it,
// and a row shows the color, the blend and the position of the selected stop (Gradient Map).

import { useEffect, useRef, useState, type CSSProperties, type KeyboardEvent, type PointerEvent } from 'react';
import { Field } from './Field';
import { BLENDS, rampGradient, type Stop } from './gui';
import { clamp, ColorInput, own, round, Titled, type Input } from './ValueWidgets';

export function ColorRamp({ w, send, disabled }: Input<'ramp'>) {
  const [drag, setDrag] = useState<{ i: number; at: number } | null>(null);
  const area = useRef<HTMLDivElement>(null);
  const stops = w.stops.map((s, i): Stop => (drag?.i === i ? [drag.at, s[1]] : s));
  const selected = drag?.i ?? Math.min(w.selected, stops.length - 1);
  const stop = stops[selected];
  const name = w.text || 'Color ramp';
  // A move can put the stop at another index, so the focus follows the selected stop.
  useEffect(() => {
    const focused = document.activeElement;
    if (focused?.hasAttribute('data-stop') && area.current?.contains(focused)) area.current.querySelector<HTMLElement>(`[data-stop="${w.selected}"]`)?.focus();
  }, [w.selected]);
  const at = (e: PointerEvent) => {
    const r = area.current!.getBoundingClientRect();
    return round(clamp((e.clientX - r.left) / r.width));
  };
  const down = (e: PointerEvent<HTMLDivElement>) => {
    if (e.button !== 0 || e.detail > 1) return;
    const i = Number((e.target as Element).getAttribute('data-stop') ?? NaN);
    if (!Number.isInteger(i)) return send(`add ${at(e)}`);
    e.currentTarget.setPointerCapture(e.pointerId);
    setDrag({ i, at: w.stops[i][0] });
  };
  const move = (e: PointerEvent<HTMLDivElement>) => {
    if (drag && e.currentTarget.hasPointerCapture(e.pointerId)) setDrag({ i: drag.i, at: at(e) });
  };
  const up = () => {
    if (drag && drag.at !== w.stops[drag.i][0]) send(`move ${drag.i} ${drag.at}`);
    setDrag(null);
  };
  const key = (e: KeyboardEvent<HTMLDivElement>, i: number) => {
    const step = e.shiftKey ? 0.01 : 0.05;
    const d = { ArrowLeft: -step, ArrowDown: -step, ArrowRight: step, ArrowUp: step }[e.key];
    if (e.key === 'Delete' || e.key === 'Backspace') {
      own(e);
      send(`remove ${i}`);
    } else if (d) {
      own(e);
      send(`move ${i} ${round(clamp(w.stops[i][0] + d))}`);
    }
  };
  return (
    <Titled text={w.text}>
      <div
        ref={area}
        className="gui-ramp"
        role="group"
        aria-label={`${name}: click to add a stop`}
        onPointerDown={disabled ? undefined : down}
        onPointerMove={disabled ? undefined : move}
        onPointerUp={disabled ? undefined : up}
        onPointerCancel={() => setDrag(null)}
      >
        <div className="ramp-bar" style={{ background: rampGradient(stops, w.blend) }} />
        {stops.map(([v, color], i) => (
          <div
            key={i}
            data-stop={i}
            className={i === selected ? 'ramp-stop selected' : 'ramp-stop'}
            style={{ left: `${v * 100}%`, '--stop': color } as CSSProperties}
            role="slider"
            tabIndex={disabled ? -1 : 0}
            aria-label={`${name} stop ${i + 1}, ${color}`}
            aria-valuemin={0}
            aria-valuemax={1}
            aria-valuenow={v}
            aria-disabled={disabled}
            onFocus={disabled ? undefined : () => i !== w.selected && send(`select ${i}`)}
            onKeyDown={disabled ? undefined : (e) => key(e, i)}
            onDoubleClick={disabled ? undefined : () => send(`remove ${i}`)}
          />
        ))}
      </div>
      {stop && (
        <div className="gui-ramp-stop">
          <ColorInput value={stop[1]} label={`${name} stop ${selected + 1} color`} send={(c) => send(`color ${selected} ${c}`)} disabled={disabled} />
          <span className="gui-value">{stop[1]}</span>
          <select className="field" aria-label={`${name} blend`} value={w.blend} disabled={disabled} onChange={(e) => send(`blend ${e.currentTarget.value}`)}>
            {BLENDS.map((b) => (
              <option key={b} value={b}>
                {b[0].toUpperCase() + b.slice(1)}
              </option>
            ))}
          </select>
          <Field w={{ kind: 'number', index: w.index, text: 'Pos', value: stop[0], step: 0.01, unit: '' }} send={(v) => send(`move ${selected} ${round(clamp(Number(v)))}`)} disabled={disabled} />
        </div>
      )}
    </Titled>
  );
}

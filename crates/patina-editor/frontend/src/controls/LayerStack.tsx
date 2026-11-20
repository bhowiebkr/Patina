// The layer stack of a node GUI (spec 4.3): ordered rows with a handle, a visibility toggle, a
// swatch, the name and the amount.

import { useEffect, useRef, useState, type KeyboardEvent, type MouseEvent, type PointerEvent } from 'react';
import { Field } from './Field';
import { guiIcon } from './icons';
import { clamp, own, round, Titled, type Input } from './ValueWidgets';

export function LayerStack({ w, send, disabled }: Input<'layers'>) {
  const [drag, setDrag] = useState<{ from: number; to: number } | null>(null);
  const list = useRef<HTMLDivElement>(null);
  const n = w.layers.length;
  const order = w.layers.map((_, i) => i);
  if (drag) order.splice(drag.to, 0, ...order.splice(drag.from, 1));
  const selected = drag?.from ?? Math.min(w.selected, n - 1);
  // A move puts the layer at another index, so the focus follows the selected layer.
  useEffect(() => {
    const focused = document.activeElement;
    if (focused?.hasAttribute('data-layer') && list.current?.contains(focused)) list.current.querySelector<HTMLElement>(`[data-layer="${w.selected}"]`)?.focus();
  }, [w.selected]);
  // The list holds the capture: React moves the row of the drag, and a move of the handle releases its capture.
  const down = (e: PointerEvent<HTMLButtonElement>, i: number) => {
    if (e.button !== 0) return;
    list.current!.setPointerCapture(e.pointerId);
    setDrag({ from: i, to: i });
  };
  const move = (e: PointerEvent<HTMLDivElement>) => {
    if (!drag) return;
    const r = list.current!.getBoundingClientRect();
    const to = Math.min(Math.max(Math.floor(((e.clientY - r.top) / r.height) * n), 0), n - 1);
    if (to !== drag.to) setDrag({ from: drag.from, to });
  };
  const up = () => {
    if (drag && drag.to !== drag.from) send(`move ${drag.from} ${drag.to}`);
    setDrag(null);
  };
  const key = (e: KeyboardEvent<HTMLButtonElement>, i: number) => {
    const to = { ArrowUp: i - 1, ArrowDown: i + 1 }[e.key];
    if (to === undefined) return;
    own(e);
    if (to >= 0 && to < n) send(`move ${i} ${to}`);
  };
  const click = (e: MouseEvent, i: number) => {
    if (i !== selected && !(e.target as Element).closest('button, label')) send(`select ${i}`);
  };
  return (
    <Titled text={w.text}>
      <div
        ref={list}
        className="gui-layers"
        role="list"
        aria-label={w.text || 'Layers'}
        onPointerMove={move}
        onPointerUp={up}
        onPointerCancel={() => setDrag(null)}
        onLostPointerCapture={() => setDrag(null)}
      >
        {order.map((i) => {
          const l = w.layers[i];
          const name = l.name || `Layer ${i + 1}`;
          const rowClass = ['gui-layer', i === selected && 'selected', !l.visible && 'hidden', drag?.from === i && 'dragged'].filter(Boolean).join(' ');
          return (
            <div key={i} role="listitem" className={rowClass} onClick={disabled ? undefined : (e) => click(e, i)}>
              <button
                className="icon-btn layer-handle"
                data-layer={i}
                aria-label={`Move ${name}`}
                title="Drag, or press the up and down arrows"
                disabled={disabled}
                onPointerDown={(e) => down(e, i)}
                onKeyDown={(e) => key(e, i)}
                onFocus={() => i !== w.selected && send(`select ${i}`)}
              >
                {guiIcon('grip')}
              </button>
              <button
                className="icon-btn layer-eye"
                aria-pressed={l.visible}
                aria-label={`Show ${name}`}
                title={l.visible ? 'Hide' : 'Show'}
                disabled={disabled}
                onClick={() => send(`visible ${i} ${l.visible ? 0 : 1}`)}
              >
                {guiIcon(l.visible ? 'eye' : 'eye-off')}
              </button>
              <span className="layer-swatch" style={{ background: l.color }} />
              <button className="layer-name" aria-pressed={i === selected} disabled={disabled} onClick={() => i !== selected && send(`select ${i}`)}>
                {name}
              </button>
              {l.amount === null ? (
                <span />
              ) : (
                <Field w={{ kind: 'number', index: w.index, text: `${name} amount`, value: l.amount, step: 0.01, unit: '' }} send={(v) => send(`amount ${i} ${round(clamp(Number(v)))}`)} disabled={disabled} />
              )}
            </div>
          );
        })}
      </div>
    </Titled>
  );
}

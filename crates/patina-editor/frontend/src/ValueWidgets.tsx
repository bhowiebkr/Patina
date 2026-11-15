// The value widgets of a node GUI (spec 4.3): color picker, plot, waveform, table, curve and
// image, with the shared controls of the GUI guide.

import { useEffect, useRef, useState, type KeyboardEvent, type PointerEvent, type ReactNode } from 'react';
import type { Widget } from './gui';

type Of<K extends Widget['kind']> = Extract<Widget, { kind: K }>;
interface Input<K extends Widget['kind']> {
  w: Of<K>;
  send: (value: number | string) => void;
  disabled: boolean;
}

const round = (v: number) => Math.round(v * 1000) / 1000;
const clamp = (v: number) => Math.min(Math.max(v, 0), 1);

/** A key that a widget uses does not also select, move or delete the node (GraphView, React Flow). */
function own(e: KeyboardEvent) {
  e.preventDefault();
  e.stopPropagation();
}

function Titled({ text, children }: { text: string; children: ReactNode }) {
  return (
    <div className="gui-canvas-row">
      {text && <span className="label">{text}</span>}
      {children}
    </div>
  );
}

/** The native change event comes when the picker closes, so a pick runs the graph one time. */
export function ColorPicker({ w, send, disabled }: Input<'color'>) {
  const ref = useRef<HTMLInputElement>(null);
  useEffect(() => {
    const input = ref.current;
    if (!input) return;
    if (document.activeElement !== input) input.value = w.value;
    const change = () => input.value !== w.value && send(input.value);
    input.addEventListener('change', change);
    return () => input.removeEventListener('change', change);
  }, [w.value, send]);
  return (
    <label className="gui-row">
      <span className="label">{w.text}</span>
      <input ref={ref} type="color" className="gui-color" defaultValue={w.value} disabled={disabled} />
      <span className="gui-value">{w.value}</span>
    </label>
  );
}

/** The path of a line through `ys` in a box of `width` x 1. A value that is not finite makes a gap. */
function linePath(ys: number[], width: number): string {
  let path = '';
  let gap = true;
  ys.forEach((y, i) => {
    if (!Number.isFinite(y)) return void (gap = true);
    path += `${gap ? 'M' : 'L'}${(i * width) / Math.max(ys.length - 1, 1)} ${y}`;
    gap = false;
  });
  return path;
}

export function Plot({ w }: { w: Of<'plot'> }) {
  const span = w.high - w.low || 1;
  // A margin of 8% keeps the peaks of the trace inside the box.
  const ys = w.values.map((v) => 0.08 + (0.84 * (w.high - v)) / span);
  return (
    <Titled text={w.text}>
      <div className="gui-chart">
        <svg viewBox="0 0 1 1" preserveAspectRatio="none" role="img" aria-label={`${w.text || 'Plot'}: ${w.values.length} values from ${w.low} to ${w.high}`}>
          <path className="grid" d="M0 0.5H1" />
          <path className="trace" d={linePath(ys, 1)} />
        </svg>
        <span className="axis top">{w.high}</span>
        <span className="axis bottom">{w.low}</span>
      </div>
    </Titled>
  );
}

/** The columns go from -1 at the bottom to 1 at the top. A column that is not finite is silent. */
export function Waveform({ w }: { w: Of<'waveform'> }) {
  const n = w.columns.length;
  const y = (v: number) => (Number.isFinite(v) ? 1 - Math.min(Math.max(v, -1), 1) : 1);
  const top = w.columns.map(([, high], i) => `${i + 0.5} ${y(high)}`);
  const bottom = w.columns.map(([low], i) => `${i + 0.5} ${y(low)}`).reverse();
  return (
    <Titled text={w.text}>
      <div className="gui-chart">
        <svg viewBox={`0 0 ${Math.max(n, 1)} 2`} preserveAspectRatio="none" role="img" aria-label={w.text || 'Waveform'}>
          <path className="grid" d={`M0 1H${n}`} />
          {n > 0 && <polygon className="wave" points={[...top, ...bottom].join(' ')} />}
          {w.playhead !== null && <path className="playhead" d={`M${w.playhead * n} 0V2`} />}
        </svg>
      </div>
    </Titled>
  );
}

export function Table({ w, send, disabled }: Input<'table'>) {
  const key = (e: KeyboardEvent<HTMLTableRowElement>, row: number) => {
    const next = e.key === 'ArrowDown' ? e.currentTarget.nextElementSibling : e.key === 'ArrowUp' ? e.currentTarget.previousElementSibling : null;
    if (next instanceof HTMLElement) {
      own(e);
      next.focus();
    } else if ((e.key === 'Enter' || e.key === ' ') && !disabled) {
      own(e);
      send(row);
    }
  };
  return (
    <Titled text={w.text}>
      <div className="gui-table-box">
        <table className="gui-table" role="grid" aria-label={w.text || 'Table'}>
          <thead>
            <tr>
              {w.headings.map((h, i) => (
                <th key={i}>{h}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {w.rows.map((cells, r) => (
              <tr key={r} tabIndex={0} aria-selected={w.selected === r} onClick={disabled ? undefined : () => send(r)} onKeyDown={(e) => key(e, r)}>
                {w.headings.map((_, c) => (
                  <td key={c}>{cells[c] ?? ''}</td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </Titled>
  );
}

// The curve box in SVG units. The element scales it to the width of the GUI. The margin keeps
// the handles at the edges inside the box.
const CW = 200;
const CH = 120;
const M = 8;
const px = (x: number) => M + x * (CW - 2 * M);
const py = (y: number) => M + (1 - y) * (CH - 2 * M);

/**
 * A click on an empty place adds a point, a drag moves a point and a double click removes it. A
 * focused point moves with the arrow keys and Delete removes it. A drag sends one move at its end.
 */
export function Curve({ w, send, disabled }: Input<'curve'>) {
  const [selected, setSelected] = useState<number | null>(null);
  const [drag, setDrag] = useState<{ i: number; at: [number, number] } | null>(null);
  const svg = useRef<SVGSVGElement>(null);
  const points = w.points.map((p, i): [number, number] => (drag?.i === i ? drag.at : p));
  const at = (e: PointerEvent): [number, number] => {
    const r = svg.current!.getBoundingClientRect();
    const x = ((e.clientX - r.left) / r.width) * CW;
    const y = ((e.clientY - r.top) / r.height) * CH;
    return [round(clamp((x - M) / (CW - 2 * M))), round(clamp(1 - (y - M) / (CH - 2 * M)))];
  };
  const down = (e: PointerEvent<SVGSVGElement>) => {
    if (e.button !== 0 || e.detail > 1) return;
    const i = Number((e.target as Element).getAttribute('data-point') ?? NaN);
    if (Number.isInteger(i)) {
      e.currentTarget.setPointerCapture(e.pointerId);
      setSelected(i);
      setDrag({ i, at: w.points[i] });
    } else {
      setSelected(null);
      send(`add ${at(e).join(' ')}`);
    }
  };
  const move = (e: PointerEvent<SVGSVGElement>) => {
    if (drag && e.currentTarget.hasPointerCapture(e.pointerId)) setDrag({ i: drag.i, at: at(e) });
  };
  const up = () => {
    if (drag && (drag.at[0] !== w.points[drag.i][0] || drag.at[1] !== w.points[drag.i][1])) send(`move ${drag.i} ${drag.at.join(' ')}`);
    setDrag(null);
  };
  const key = (e: KeyboardEvent<SVGCircleElement>, i: number) => {
    const step = e.shiftKey ? 0.01 : 0.05;
    const [dx, dy] = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, step], ArrowDown: [0, -step] }[e.key] ?? [0, 0];
    if (e.key === 'Delete' || e.key === 'Backspace') {
      own(e);
      setSelected(null);
      send(`remove ${i}`);
    } else if (dx || dy) {
      own(e);
      send(`move ${i} ${round(clamp(w.points[i][0] + dx))} ${round(clamp(w.points[i][1] + dy))}`);
    }
  };
  const shown = selected !== null ? points[selected] : undefined;
  return (
    <Titled text={w.text}>
      <svg
        ref={svg}
        className="gui-curve"
        viewBox={`0 0 ${CW} ${CH}`}
        role="group"
        aria-label={`${w.text || 'Curve'}: click to add a point`}
        onPointerDown={disabled ? undefined : down}
        onPointerMove={disabled ? undefined : move}
        onPointerUp={disabled ? undefined : up}
        onPointerCancel={() => setDrag(null)}
      >
        <path className="grid" d={`M${CW / 2} 0V${CH}M0 ${CH / 2}H${CW}`} />
        <polyline className="trace" points={points.map(([x, y]) => `${px(x)},${py(y)}`).join(' ')} />
        {points.map(([x, y], i) => (
          <circle
            key={i}
            data-point={i}
            className={selected === i ? 'handle selected' : 'handle'}
            cx={px(x)}
            cy={py(y)}
            r={5}
            tabIndex={disabled ? -1 : 0}
            role="button"
            aria-label={`Point ${i + 1}: x ${x}, y ${y}`}
            onFocus={() => setSelected(i)}
            onKeyDown={(e) => key(e, i)}
            onDoubleClick={disabled ? undefined : () => send(`remove ${i}`)}
          />
        ))}
      </svg>
      <span className="gui-hint">{shown ? `x ${shown[0].toFixed(3)}   y ${shown[1].toFixed(3)}` : `${points.length} points`}</span>
    </Titled>
  );
}

/** Draws the pixels of the image when they change. An empty image shows an empty box. */
export function Picture({ w }: { w: Of<'image'> }) {
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const context = ref.current?.getContext('2d');
    if (!context || w.width === 0 || w.height === 0) return;
    const bytes = Uint8ClampedArray.from(atob(w.rgba), (c) => c.charCodeAt(0));
    context.putImageData(new ImageData(bytes, w.width, w.height), 0, 0);
  }, [w.rgba, w.width, w.height]);
  return (
    <Titled text={w.text}>
      {w.width > 0 && w.height > 0 ? (
        <canvas ref={ref} className="gui-image" width={w.width} height={w.height} role="img" aria-label={`${w.text || 'Image'}: ${w.width} x ${w.height} pixels`} />
      ) : (
        <div className="gui-image empty">No image</div>
      )}
    </Titled>
  );
}

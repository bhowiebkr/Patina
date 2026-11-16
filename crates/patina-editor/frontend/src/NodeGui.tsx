// Draws the widget lines of a node GUI with the shared controls (spec 4.3, GUI guide controls).

import { createContext, memo, useContext, useEffect, useRef, useState, type CSSProperties, type PointerEvent } from 'react';
import { api } from './api';
import { eventLine, parseTree, type Shape, type Widget } from './gui';
import { ColorPicker, Curve, Picture, Plot, Table, Waveform } from './ValueWidgets';
import { ViewportView } from './Viewport';

interface Props {
  id: string;
  /** 0 for the node body, 1 for the Inspector, 2 for the full size. */
  mode: number;
  tree: string;
  disabled: boolean;
}

function Slider({ w, send, disabled }: { w: Extract<Widget, { kind: 'slider' }>; send: (value: number) => void; disabled: boolean }) {
  // A drag shows its own value and sends it at the end, so a drag runs the graph one time (9.1).
  const [draft, setDraft] = useState<number | null>(null);
  const value = draft ?? w.value;
  const fill = w.high > w.low ? Math.min(100, Math.max(0, ((value - w.low) / (w.high - w.low)) * 100)) : 0;
  const commit = () => {
    if (draft !== null && draft !== w.value) send(draft);
    setDraft(null);
  };
  return (
    <label className="gui-row">
      <span className="label">{w.text}</span>
      <input
        type="range"
        className="gui-slider"
        min={w.low}
        max={w.high}
        step="any"
        value={value}
        style={{ '--fill': `${fill}%` } as CSSProperties}
        disabled={disabled}
        onChange={(e) => setDraft(Number(e.currentTarget.value))}
        onPointerUp={commit}
        onKeyUp={commit}
        onBlur={commit}
      />
      <span className="gui-value">{Number(value.toPrecision(4))}</span>
    </label>
  );
}

function ShapeView({ s }: { s: Shape }) {
  switch (s.kind) {
    case 'line':
      return <line x1={s.x1} y1={s.y1} x2={s.x2} y2={s.y2} stroke={s.color} strokeWidth={1.5} />;
    case 'rect':
      return <rect x={s.x} y={s.y} width={s.w} height={s.h} stroke={s.color} fill={s.fill ? s.color : 'none'} />;
    case 'circle':
      return <circle cx={s.x} cy={s.y} r={s.r} stroke={s.color} fill={s.fill ? s.color : 'none'} />;
    case 'text':
      return (
        <text x={s.x} y={s.y} fill={s.color} fontSize={12}>
          {s.text}
        </text>
      );
  }
}

/** A canvas sends a pointer down, the moves of a drag (one for each frame at most) and the up. */
function CanvasView({ w, send, disabled }: { w: Extract<Widget, { kind: 'canvas' }>; send: (value: string) => void; disabled: boolean }) {
  const pending = useRef<string | null>(null);
  // The point in canvas pixels. A zoom of the graph scales the element, so divide by its size.
  const at = (e: PointerEvent<SVGSVGElement>) => {
    const r = e.currentTarget.getBoundingClientRect();
    const round = (v: number) => Math.round(v * 10) / 10;
    return `${round(((e.clientX - r.left) * w.width) / r.width)} ${round(((e.clientY - r.top) * w.height) / r.height)}`;
  };
  const flush = () => {
    if (pending.current !== null) send(`move ${pending.current}`);
    pending.current = null;
  };
  const down = (e: PointerEvent<SVGSVGElement>) => {
    e.currentTarget.setPointerCapture(e.pointerId);
    send(`down ${at(e)}`);
  };
  const move = (e: PointerEvent<SVGSVGElement>) => {
    if (!e.currentTarget.hasPointerCapture(e.pointerId)) return;
    if (pending.current === null) requestAnimationFrame(flush);
    pending.current = at(e);
  };
  const up = (e: PointerEvent<SVGSVGElement>) => {
    if (!e.currentTarget.hasPointerCapture(e.pointerId)) return;
    flush();
    send(`up ${at(e)}`);
  };
  return (
    <div className="gui-canvas-row">
      {w.text && (
        <span className="label" title={w.text}>
          {w.text}
        </span>
      )}
      <svg
        className="gui-canvas"
        width={w.width}
        height={w.height}
        style={{ aspectRatio: `${w.width} / ${w.height}` }}
        viewBox={`0 0 ${w.width} ${w.height}`}
        role="img"
        aria-label={w.text || 'Canvas'}
        onPointerDown={disabled ? undefined : down}
        onPointerMove={disabled ? undefined : move}
        onPointerUp={disabled ? undefined : up}
      >
        {w.shapes.map((s, i) => (
          <ShapeView key={i} s={s} />
        ))}
      </svg>
    </div>
  );
}

/** A field that sends its value on Enter or when it loses the focus, and Esc restores it. */
function Field({ w, send, disabled }: { w: Extract<Widget, { kind: 'number' | 'text' }>; send: (value: number | string) => void; disabled: boolean }) {
  const shown = String(w.value);
  const [draft, setDraft] = useState<string | null>(null);
  const commit = () => {
    if (draft === null) return;
    setDraft(null);
    if (draft === shown) return;
    if (w.kind === 'text') send(draft);
    else if (draft.trim() !== '' && Number.isFinite(Number(draft))) send(Number(draft));
  };
  return (
    <label className="gui-row">
      <span className="label">{w.text}</span>
      <input
        className="field gui-field"
        inputMode={w.kind === 'number' ? 'decimal' : undefined}
        value={draft ?? shown}
        disabled={disabled}
        onChange={(e) => setDraft(e.currentTarget.value)}
        onBlur={commit}
        onKeyDown={(e) => {
          if (e.key === 'Enter') commit();
          else if (e.key === 'Escape') setDraft(null);
        }}
      />
    </label>
  );
}

// The events of all node GUIs go out in order, so the down of a drag comes before its up.
let queue = Promise.resolve();

function NodeGuiImpl({ id, mode, tree, disabled }: Props) {
  const send = (line: string) => {
    queue = queue.then(() => api.gui(id, mode, line)).catch(() => {});
  };
  return <GuiWidgets tree={tree} disabled={disabled} label="Node controls" send={send} />;
}

interface WidgetsProps {
  tree: string;
  disabled: boolean;
  label: string;
  send: (line: string) => void;
  /** The native window handle for a viewport. Only a view gives it in version 1 (15.8). */
  window?: number;
}

/** Draws widget lines. `send` gets the event line of each user input. A view uses it too (15.8). */
export function GuiWidgets({ tree, disabled, label, send: sendLine, window = 0 }: WidgetsProps) {
  const send = (index: number) => (value: number | string) => sendLine(eventLine(index, value));
  return (
    // The nodrag and nowheel classes keep a drag on a control from moving the node (GUI guide).
    <div className="node-gui nodrag nowheel" role="group" aria-label={label}>
      {parseTree(tree).map((w) => {
        switch (w.kind) {
          case 'label':
            return (
              <div key={w.index} className="gui-row gui-label" title={w.text}>
                {w.text}
              </div>
            );
          case 'button':
            return (
              <button key={w.index} className="btn gui-button" disabled={disabled} onClick={() => send(w.index)(1)}>
                {w.text}
              </button>
            );
          case 'toggle':
            return (
              <label key={w.index} className="gui-row">
                <input type="checkbox" checked={w.on} disabled={disabled} onChange={(e) => send(w.index)(e.currentTarget.checked ? 1 : 0)} />
                <span>{w.text}</span>
              </label>
            );
          case 'slider':
            return <Slider key={w.index} w={w} send={send(w.index)} disabled={disabled} />;
          case 'number':
          case 'text':
            return <Field key={w.index} w={w} send={send(w.index)} disabled={disabled} />;
          case 'dropdown':
            return (
              <label key={w.index} className="gui-row">
                <span className="label">{w.text}</span>
                <select className="field gui-field" value={w.selected} disabled={disabled} onChange={(e) => send(w.index)(Number(e.currentTarget.value))}>
                  {w.options.map((o, i) => (
                    <option key={i} value={i}>
                      {o}
                    </option>
                  ))}
                </select>
              </label>
            );
          case 'canvas':
            return <CanvasView key={w.index} w={w} send={send(w.index)} disabled={disabled} />;
          case 'viewport':
            return <ViewportView key={w.index} w={w} send={send(w.index)} disabled={disabled} window={window} />;
          case 'color':
            return <ColorPicker key={w.index} w={w} send={send(w.index)} disabled={disabled} />;
          case 'plot':
            return <Plot key={w.index} w={w} />;
          case 'waveform':
            return <Waveform key={w.index} w={w} />;
          case 'table':
            return <Table key={w.index} w={w} send={send(w.index)} disabled={disabled} />;
          case 'curve':
            return <Curve key={w.index} w={w} send={send(w.index)} disabled={disabled} />;
          case 'image':
            return <Picture key={w.index} w={w} />;
        }
      })}
    </div>
  );
}

export const NodeGui = memo(NodeGuiImpl);

/** Opens the full size of the GUI of a node (4.3). The app gives it. */
export const ExpandGui = createContext<(id: string) => void>(() => {});

/** The expand button of a node with `full_gui`, in the node header and the Inspector header. */
export function ExpandButton({ id }: { id: string }) {
  const expand = useContext(ExpandGui);
  return (
    <button className="icon-btn expand nodrag" aria-label="Open the full size" title="Full size" onClick={() => expand(id)}>
      <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" aria-hidden="true">
        <path d="M9 2h5v5M7 14H2V9M14 2 9 7M2 14l5-5" />
      </svg>
    </button>
  );
}

/** The full size of a node GUI over the graph area (4.3). Esc closes it. */
export function FullGui({ id, tree, disabled, onBack }: { id: string; tree: string; disabled: boolean; onBack: () => void }) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    ref.current?.focus();
    api.gui(id, 2, '').catch(() => {});
    // An empty ID tells the engine to stop the draws of the full size.
    return () => void api.gui('', 2, '').catch(() => {});
  }, [id]);
  return (
    <div
      ref={ref}
      className="full-gui"
      tabIndex={-1}
      role="region"
      aria-label="Full size GUI"
      onKeyDown={(e) => {
        // The keys do not change the graph under the GUI. The Ctrl keys still reach the app.
        if (!e.ctrlKey && !e.metaKey) e.stopPropagation();
        const field = e.target instanceof HTMLInputElement || e.target instanceof HTMLSelectElement;
        if (e.key === 'Escape' && !field) onBack();
      }}
    >
      <NodeGui id={id} mode={2} tree={tree} disabled={disabled} />
    </div>
  );
}

// Draws the widget lines of a node GUI with the shared controls (spec 4.3, GUI guide controls).

import { Component, createContext, memo, useContext, useEffect, useRef, useState, type CSSProperties, type PointerEvent, type ReactNode } from 'react';
import { api, holdGuiGesture } from '../api';
import { engineId } from '../canvas/crumbs';
import { AssetPicker, CodeField, Field } from './Field';
import { eventLine, freeLayout, nest, parseTree, type Block, type Shape, type Widget } from './gui';
import { guiIcon } from './icons';
import { LayerStack } from './LayerStack';
import { ColorRamp } from './Ramp';
import { Timeline } from './Timeline';
import { TransformGizmo } from './Transform';
import { ColorPicker, Curve, Picture, Plot, Progress, Table, Waveform } from './ValueWidgets';
import { useKeys, ViewportView } from './Viewport';

interface Props {
  id: string;
  /** 0 for the node body, 1 for the Inspector, 2 for the full size. */
  mode: number;
  tree: string;
  disabled: boolean;
}

function Slider({ w, send, disabled }: { w: Extract<Widget, { kind: 'slider' }>; send: (value: number) => void; disabled: boolean }) {
  // A drag sends its value at most one time in each display frame, so the node and its preview change during
  // the drag. The values of one drag are one step of undo (9.1).
  const [draft, setDraft] = useState<number | null>(null);
  const pending = useRef<number | null>(null);
  const frame = useRef(0);
  const held = useRef(false);
  const value = draft ?? w.value;
  const fill = w.high > w.low ? Math.min(100, Math.max(0, ((value - w.low) / (w.high - w.low)) * 100)) : 0;
  const flush = () => {
    cancelAnimationFrame(frame.current);
    frame.current = 0;
    if (pending.current !== null && pending.current !== w.value) send(pending.current);
    pending.current = null;
  };
  const change = (v: number) => {
    setDraft(v);
    pending.current = v;
    if (!frame.current) frame.current = requestAnimationFrame(flush);
  };
  const hold = (on: boolean) => {
    if (held.current !== on) holdGuiGesture(on);
    held.current = on;
  };
  const commit = () => {
    flush();
    hold(false);
    setDraft(null);
  };
  // A slider that goes away during a drag ends the drag, so the next input is a new step of undo.
  useEffect(() => () => {
    cancelAnimationFrame(frame.current);
    if (held.current) holdGuiGesture(false);
  }, []);
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
        onChange={(e) => change(Number(e.currentTarget.value))}
        onPointerDown={() => hold(true)}
        onPointerUp={commit}
        onPointerCancel={commit}
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
    case 'path':
      return <path d={s.d} stroke={s.color} fill={s.fill ? s.color : 'none'} />;
  }
}

/** A press on a canvas of a graph node waits this long for a drag before the canvas takes it. */
const HOLD_MS = 250;
/** A pointer that moves this far in screen pixels during the wait starts a drag of the node. */
const DRAG_PX = 4;
/** A placed widget of these kinds fills the width from its x to the same margin at the right edge (4.3). */
const FILLS = new Set(['image', 'number', 'text', 'path', 'dropdown']);

/** A canvas sends a pointer down, the moves of a drag (one for each frame at most) and the up. A
 * key canvas also takes the focus on a click and sends the keys, as a viewport does. On a graph
 * node, a canvas that does not grab gives a drag to the node, and sends the down at the release
 * or after a short hold (4.3). */
function CanvasView({ w, send, disabled }: { w: Extract<Widget, { kind: 'canvas' }>; send: (value: string) => void; disabled: boolean }) {
  const pending = useRef<string | null>(null);
  const held = useRef(false);
  const lastPoint = useRef('0 0');
  const press = useRef<{ x: number; y: number; at: string; node: Element; timer: number } | null>(null);
  useEffect(() => () => clearTimeout(press.current?.timer), []);
  const keys = useKeys(send);
  const live = w.keys && !disabled;
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
  const take = () => {
    const p = press.current;
    if (!p) return;
    clearTimeout(p.timer);
    press.current = null;
    held.current = true;
    send(`down ${p.at}`);
  };
  const drop = () => {
    pending.current = null;
    if (held.current) send(`up ${lastPoint.current}`);
    clearTimeout(press.current?.timer);
    press.current = null;
    held.current = false;
  };
  const down = (e: PointerEvent<SVGSVGElement>) => {
    lastPoint.current = at(e);
    e.currentTarget.setPointerCapture(e.pointerId);
    if (live) e.currentTarget.focus();
    const [px, py] = lastPoint.current.split(' ').map(Number);
    const grabbed = w.grab || w.grabs.some(([x, y, gw, gh]) => px >= x && px <= x + gw && py >= y && py <= y + gh);
    const node = grabbed || e.button !== 0 ? null : e.currentTarget.closest('.react-flow__node');
    if (node) {
      press.current = { x: e.clientX, y: e.clientY, at: at(e), node, timer: window.setTimeout(take, HOLD_MS) };
      return;
    }
    held.current = true;
    send(`down ${at(e)}`);
  };
  const move = (e: PointerEvent<SVGSVGElement>) => {
    lastPoint.current = at(e);
    const p = press.current;
    if (p) {
      if (Math.hypot(e.clientX - p.x, e.clientY - p.y) < DRAG_PX) return;
      drop();
      e.currentTarget.releasePointerCapture(e.pointerId);
      // React Flow starts a node drag on a mouse down outside of a nodrag element. The real moves and the up continue it.
      const start = { bubbles: true, cancelable: true, view: window, button: 0, buttons: 1, clientX: p.x, clientY: p.y };
      p.node.dispatchEvent(new MouseEvent('mousedown', start));
      return;
    }
    if (!held.current) return;
    if (pending.current === null) requestAnimationFrame(flush);
    pending.current = at(e);
  };
  const up = (e: PointerEvent<SVGSVGElement>) => {
    take();
    if (!held.current) return;
    held.current = false;
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
        role={w.keys ? 'application' : 'img'}
        aria-label={w.text || 'Canvas'}
        tabIndex={live ? 0 : undefined}
        {...(live ? keys : w.keys ? { onBlur: keys.onBlur } : {})}
        onBlur={() => { keys.onBlur(); drop(); }}
        onPointerDown={disabled ? undefined : down}
        onPointerMove={disabled ? undefined : move}
        onPointerUp={disabled ? undefined : up}
        onPointerCancel={drop}
        onLostPointerCapture={drop}
      >
        {w.shapes.map((s, i) => (
          <ShapeView key={i} s={s} />
        ))}
      </svg>
    </div>
  );
}

/** A row of tabs (controls guide). The node shows the widgets of the selected tab after it. */
function Tabs({ w, send, disabled }: { w: Extract<Widget, { kind: 'tabs' }>; send: (value: number) => void; disabled: boolean }) {
  return (
    <div className="tabs gui-tabs" role="tablist" aria-label={w.text || 'Tabs'}>
      {w.options.map((o, i) => (
        <button key={i} role="tab" aria-selected={i === w.selected} className={i === w.selected ? 'tab active' : 'tab'} disabled={disabled} onClick={() => i !== w.selected && send(i)}>
          {o}
        </button>
      ))}
    </div>
  );
}

/** A section that the user opens and closes. The node keeps the state, so a click sends it. */
function Section({ w, send, disabled, children }: { w: Extract<Widget, { kind: 'section' }>; send: (value: number) => void; disabled: boolean; children: ReactNode }) {
  return (
    <div className="gui-section">
      <button className="gui-section-head" aria-expanded={w.open} disabled={disabled} onClick={() => send(w.open ? 0 : 1)}>
        <svg className="chevron" width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1.5" aria-hidden="true">
          <path d="M3.5 2 6.5 5l-3 3" />
        </svg>
        <span>{w.text}</span>
      </button>
      {w.open && <div className="gui-section-body">{children}</div>}
    </div>
  );
}

// The events of all node GUIs go out in order, so the down of a drag comes before its up.
let queue = Promise.resolve();

function NodeGuiImpl({ id, mode, tree, disabled }: Props) {
  const scope = useContext(GuiScope);
  const send = (line: string) => {
    queue = queue.then(() => api.gui(engineId(scope, id), mode, line)).catch(() => {});
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
export function GuiWidgets(props: WidgetsProps) {
  return (
    <Guard tree={props.tree} label={props.label}>
      <Widgets {...props} />
    </Guard>
  );
}

interface GuardProps {
  tree: string;
  label: string;
  children: ReactNode;
}

/** Keeps an error in the drawing of one GUI in that GUI, so a failed node does not stop the editor
 * (16.1). The next tree draws again. */
export class Guard extends Component<GuardProps, { error: string; tree: string }> {
  state = { error: '', tree: this.props.tree };

  static getDerivedStateFromError(error: unknown) {
    return { error: error instanceof Error ? error.message : String(error) };
  }

  static getDerivedStateFromProps(props: GuardProps, state: { tree: string }) {
    return props.tree === state.tree ? null : { error: '', tree: props.tree };
  }

  render() {
    if (!this.state.error) return this.props.children;
    return (
      <div className="node-gui" role="group" aria-label={this.props.label}>
        <div className="gui-row error-text" role="alert">
          The GUI cannot draw: {this.state.error.slice(0, 200)}
        </div>
      </div>
    );
  }
}

function Widgets({ tree, disabled, label, send: sendLine, window = 0 }: WidgetsProps) {
  const send = (index: number) => (value: number | string) => sendLine(eventLine(index, value));
  const blocks = nest(parseTree(tree));
  // A placed widget (4.3) is on top of the column, at its point.
  const places = tree.includes('place\t') ? freeLayout(tree).places : null;
  return (
    // The nodrag class keeps a drag on a control from moving the node (GUI guide). The wheel zooms the canvas, also here (15.2).
    <div className={`node-gui nodrag${places ? ' placed' : ''}`} role="group" aria-label={label}>
      {blocks.map(function draw(w: Block): ReactNode {
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
          case 'icons':
            return (
              <div key={w.index} className="gui-icons">
                {w.items.map((b) => (
                  <button key={b.index} className="btn gui-icon" aria-label={b.text} title={b.text} disabled={disabled} onClick={() => send(b.index)(1)}>
                    {guiIcon(b.icon) ?? b.text}
                  </button>
                ))}
              </div>
            );
          case 'progress':
            return <Progress key={w.index} w={w} />;
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
          case 'path':
            return <Field key={w.index} w={w} send={send(w.index)} disabled={disabled} />;
          case 'code':
            return <CodeField key={w.index} w={w} send={send(w.index)} disabled={disabled} />;
          case 'asset':
            return <AssetPicker key={w.index} w={w} send={send(w.index)} disabled={disabled} />;
          case 'tabs':
            return <Tabs key={w.index} w={w} send={send(w.index)} disabled={disabled} />;
          case 'group':
            return <div key={w.index} className={w.minWidth ? 'gui-columns' : 'gui-column'} style={{ '--column-min': `${w.minWidth}px` } as CSSProperties}>{w.items.map(draw)}</div>;
          case 'section':
            return (
              <Section key={w.index} w={w} send={send(w.index)} disabled={disabled}>
                {w.items.map(draw)}
              </Section>
            );
          case 'dropdown':
            return (
              <label key={w.index} className="gui-row">
                <span className="label">{w.text}</span>
                <select className="field gui-field" value={w.selected} title={w.options[w.selected]} disabled={disabled} onChange={(e) => send(w.index)(Number(e.currentTarget.value))}>
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
          case 'scatter':
            return <Plot key={w.index} w={w} />;
          case 'waveform':
            return <Waveform key={w.index} w={w} send={send(w.index)} disabled={disabled} />;
          case 'table':
            return <Table key={w.index} w={w} send={send(w.index)} disabled={disabled} />;
          case 'curve':
            return <Curve key={w.index} w={w} send={send(w.index)} disabled={disabled} />;
          case 'ramp':
            return <ColorRamp key={w.index} w={w} send={send(w.index)} disabled={disabled} />;
          case 'layers':
            return <LayerStack key={w.index} w={w} send={send(w.index)} disabled={disabled} />;
          case 'image':
            return <Picture key={w.index} w={w} />;
          case 'transform':
            return <TransformGizmo key={w.index} w={w} send={send(w.index)} disabled={disabled} />;
          case 'timeline':
            return <Timeline key={w.index} w={w} send={send(w.index)} disabled={disabled} />;
        }
      }).map((node, i) => {
        const at = places?.get(blocks[i].index);
        return at ? (
          <div key={blocks[i].index} className="gui-place" style={{ left: at[0], top: at[1], right: FILLS.has(blocks[i].kind) ? at[0] : undefined }}>
            {node}
          </div>
        ) : (
          node
        );
      })}
    </div>
  );
}

export const NodeGui = memo(NodeGuiImpl);

/** Opens the full size of the GUI of a node (4.3). The app gives it. */
export const ExpandGui = createContext<(id: string) => void>(() => {});

/** The use path of the open subgraph (6.4). The engine knows a node inside a use as "<use path>/<id>". */
export const GuiScope = createContext('');

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
  const scope = useContext(GuiScope);
  useEffect(() => {
    ref.current?.focus();
    api.gui(engineId(scope, id), 2, '').catch(() => {});
    // An empty ID tells the engine to stop the draws of the full size.
    return () => void api.gui('', 2, '').catch(() => {});
  }, [id, scope]);
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
        const field = e.target instanceof HTMLInputElement || e.target instanceof HTMLSelectElement || e.target instanceof HTMLTextAreaElement;
        if (e.key === 'Escape' && !field) onBack();
      }}
    >
      <NodeGui id={id} mode={2} tree={tree} disabled={disabled} />
    </div>
  );
}

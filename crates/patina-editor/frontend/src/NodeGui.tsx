// Draws the widget lines of a node GUI with the shared controls (spec 4.3, GUI guide controls).

import { createContext, memo, useContext, useEffect, useRef, useState } from 'react';
import { api } from './api';
import { eventLine, parseTree, type Widget } from './gui';

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

function NodeGuiImpl({ id, mode, tree, disabled }: Props) {
  const send = (index: number) => (value: number | string) => void api.gui(id, mode, eventLine(index, value)).catch(() => {});
  return (
    // The nodrag and nowheel classes keep a drag on a control from moving the node (GUI guide).
    <div className="node-gui nodrag nowheel" role="group" aria-label="Node controls">
      {parseTree(tree).map((w) => {
        switch (w.kind) {
          case 'label':
            return (
              <div key={w.index} className="gui-row">
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

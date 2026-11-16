// A text or number field of a node GUI (spec 4.3, GUI guide controls).

import { useState } from 'react';
import { stepped, type Widget } from './gui';

const Icon = ({ d }: { d: string }) => (
  <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.5" aria-hidden="true">
    <path d={d} />
  </svg>
);

/** A field that sends its value on Enter or when it loses the focus, and Esc restores it. A number
 * field with a step also has step buttons, and the up and down arrows step it. */
export function Field({ w, send, disabled }: { w: Extract<Widget, { kind: 'number' | 'text' }>; send: (value: number | string) => void; disabled: boolean }) {
  const shown = String(w.value);
  const [draft, setDraft] = useState<string | null>(null);
  const commit = () => {
    if (draft === null) return;
    setDraft(null);
    if (draft === shown) return;
    if (w.kind === 'text') send(draft);
    else if (draft.trim() !== '' && Number.isFinite(Number(draft))) send(Number(draft));
  };
  const step = w.kind === 'number' ? w.step : 0;
  const by = (count: number) => {
    setDraft(null);
    if (w.kind === 'number') send(stepped(w.value, w.step, count));
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
          else if (step && (e.key === 'ArrowUp' || e.key === 'ArrowDown')) {
            e.preventDefault();
            by(e.key === 'ArrowUp' ? 1 : -1);
          }
        }}
      />
      {w.kind === 'number' && w.unit && <span className="gui-unit">{w.unit}</span>}
      {step > 0 && (
        <span className="gui-steps">
          <button className="icon-btn gui-step" aria-label={`Decrease ${w.text}`} title={`-${step}`} disabled={disabled} onClick={() => by(-1)}>
            <Icon d="M3 6h6" />
          </button>
          <button className="icon-btn gui-step" aria-label={`Increase ${w.text}`} title={`+${step}`} disabled={disabled} onClick={() => by(1)}>
            <Icon d="M3 6h6M6 3v6" />
          </button>
        </span>
      )}
    </label>
  );
}

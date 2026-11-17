// A text, number or file path field, and the asset picker, of a node GUI (spec 4.3, GUI guide controls).

import { useEffect, useState } from 'react';
import { api } from './api';
import { escapeCode, stepped, type Widget } from './gui';
import { guiIcon } from './icons';

const Icon = ({ d }: { d: string }) => (
  <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.5" aria-hidden="true">
    <path d={d} />
  </svg>
);

/** A field that sends its value on Enter or when it loses the focus, and Esc restores it. A number
 * field with a step also has step buttons, and the up and down arrows step it. A path field has a
 * **Browse** button. */
export function Field({ w, send, disabled }: { w: Extract<Widget, { kind: 'number' | 'text' | 'path' }>; send: (value: number | string) => void; disabled: boolean }) {
  const shown = String(w.value);
  const [draft, setDraft] = useState<string | null>(null);
  const commit = () => {
    if (draft === null) return;
    setDraft(null);
    if (draft === shown) return;
    if (w.kind !== 'number') send(draft);
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
      {w.kind === 'path' && (
        <button
          className="icon-btn gui-browse"
          aria-label={`Browse ${w.text}`}
          title="Browse"
          disabled={disabled}
          onClick={() => {
            setDraft(null);
            api.pickPath(w.pick, w.value).then((path) => path !== null && path !== w.value && send(path), () => {});
          }}
        >
          {guiIcon('folder')}
        </button>
      )}
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

/** A field for a text of many lines, for example a program. It sends its text when it loses the
 * focus and on Ctrl+Enter. Esc restores the text. A code editor also shows the line numbers, a band
 * on the current line and the error lines, with the error text as a tooltip. */
export function CodeField({ w, send, disabled }: { w: Extract<Widget, { kind: 'code' }>; send: (value: string) => void; disabled: boolean }) {
  const [draft, setDraft] = useState<string | null>(null);
  const [scroll, setScroll] = useState(0);
  const text = draft ?? w.value;
  const lines = text.split('\n').length;
  const commit = () => {
    if (draft === null) return;
    setDraft(null);
    if (draft !== w.value) send(escapeCode(draft));
  };
  const area = (
    <textarea
      className="field gui-code"
      rows={w.editor ? Math.min(40, Math.max(16, lines + 1)) : Math.min(24, Math.max(6, lines + 1))}
      spellCheck={false}
      wrap="off"
      value={text}
      disabled={disabled}
      onChange={(e) => setDraft(e.currentTarget.value)}
      onBlur={commit}
      onScroll={(e) => setScroll(e.currentTarget.scrollTop)}
      onKeyDown={(e) => {
        if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) commit();
        else if (e.key === 'Escape') setDraft(null);
      }}
    />
  );
  if (!w.editor) {
    return (
      <label className="gui-code-row">
        <span className="label">{w.text}</span>
        {area}
      </label>
    );
  }
  const errors = new Map<number, string>();
  for (const [line, error] of w.errors) errors.set(line, errors.has(line) ? `${errors.get(line)}\n${error}` : error);
  const mark = (i: number) => [i === w.current && 'current', errors.has(i) && 'error'].filter(Boolean).join(' ') || undefined;
  return (
    <label className="gui-code-row">
      <span className="label">{w.text}</span>
      <div className="gui-code-box">
        <div className="gui-code-gutter" aria-hidden="true">
          <div style={{ transform: `translateY(${-scroll}px)` }}>
            {Array.from({ length: lines }, (_, i) => (
              <div key={i} className={mark(i)} title={errors.get(i)}>
                {i + 1}
              </div>
            ))}
          </div>
        </div>
        {area}
        {w.current >= 0 && <div className="gui-code-line" style={{ top: 6 + w.current * 16 - scroll }} />}
      </div>
    </label>
  );
}

/** A dropdown of the project files with one of the extensions. It reads the list again when it gets
 * the focus, so a new file shows. A path that is not in the list stays as an option. */
export function AssetPicker({ w, send, disabled }: { w: Extract<Widget, { kind: 'asset' }>; send: (value: string) => void; disabled: boolean }) {
  const [files, setFiles] = useState<string[]>([]);
  const extensions = w.extensions.join(' ');
  const load = () => {
    api.assetFiles(extensions.split(' ').filter(Boolean)).then(setFiles, () => setFiles([]));
  };
  useEffect(load, [extensions]);
  const options = w.value === '' || files.includes(w.value) ? files : [w.value, ...files];
  return (
    <label className="gui-row">
      <span className="label">{w.text}</span>
      <select className="field gui-field" value={w.value} disabled={disabled} onFocus={load} onChange={(e) => send(e.currentTarget.value)}>
        <option value="">None</option>
        {options.map((f) => (
          <option key={f} value={f}>
            {f}
          </option>
        ))}
      </select>
    </label>
  );
}

// A field that asks for a name, for example of a new node folder or of a crate (spec 15.2, 15.4).

import { useId, useRef, useState, type KeyboardEvent } from 'react';

interface Props {
  /** The label and the placeholder, for example "Node name". */
  label: string;
  /** Uses the name. It fails with the problem of the name. */
  onMake: (name: string) => Promise<void>;
  onClose: () => void;
  /** The first text of the field. */
  initial?: string;
}

/** Enter uses the name. Esc or a click out of the field cancels. A problem shows under the field. */
export function NameField({ label, onMake, onClose, initial = '' }: Props) {
  const [name, setName] = useState(initial);
  const [problem, setProblem] = useState('');
  const [busy, setBusy] = useState(false);
  const field = useRef<HTMLInputElement>(null);
  const problemId = useId();
  const onKey = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key !== 'Escape' && e.key !== 'Enter') return;
    // The keys of the field do not go to the graph.
    e.preventDefault();
    e.stopPropagation();
    if (busy) return;
    if (e.key === 'Escape') return onClose();
    setBusy(true);
    onMake(name.trim()).then(onClose, (err) => {
      setBusy(false);
      setProblem(String(err));
      field.current?.focus();
    });
  };
  return (
    <div className="new-item">
      <input
        ref={field}
        className="field"
        aria-label={label}
        aria-invalid={problem !== ''}
        aria-describedby={problem ? problemId : undefined}
        aria-busy={busy}
        readOnly={busy}
        placeholder={label}
        autoFocus
        value={name}
        onChange={(e) => {
          setName(e.target.value);
          setProblem('');
        }}
        onKeyDown={onKey}
        onBlur={() => !busy && onClose()}
      />
      {busy && <div className="label">Working</div>}
      {problem && (
        <div id={problemId} className="error-text" role="alert">
          {problem}
        </div>
      )}
    </div>
  );
}

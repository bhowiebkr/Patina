// The Update dialog (spec 6.2, 15.4): it compares an imported folder with the other project.

import { useEffect, useState } from 'react';
import { api, type UpdateChanges, type UpdatePick, type View } from './api';

interface Props {
  /** The folder of the other project. */
  other: string;
  /** The imported folder, for example `nodes/blur`. */
  folder: string;
  onDone: (view: View) => void;
  onClose: () => void;
}

const STATES = { OnlyOther: 'Only in the other project', OnlyThis: 'Only in this project', Different: 'Different' };
const SIDES = { Other: 'Other project', This: 'This project' };

export function UpdateDialog({ other, folder, onDone, onClose }: Props) {
  const [changes, setChanges] = useState<UpdateChanges | null>(null);
  const [picks, setPicks] = useState<Record<string, UpdatePick>>({});
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    api.updateChanges(other, folder).then(setChanges).catch((e) => setError(String(e)));
  }, [other, folder]);

  const start = async () => {
    setBusy(true);
    setError('');
    try {
      onDone(await api.updateFolder(other, folder, picks));
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  };
  const ready = changes !== null && changes.other && changes.files.every((f) => f.side !== 'Both' || picks[f.path]);
  const name = other.split(/[\\/]/).filter(Boolean).pop() ?? other;
  return (
    <div className="backdrop" onKeyDown={(e) => e.key === 'Escape' && !busy && onClose()}>
      <div className="dialog import" role="dialog" aria-modal="true" aria-labelledby="update-title">
        <h2 id="update-title">
          Update {folder} from {name}
        </h2>
        {changes && !changes.other && <div className="hint">The folder in the other project did not change.</div>}
        {changes?.other && changes.files.some((f) => f.side === 'Both') && (
          <div className="hint">The two projects changed some files. Pick the version of each of these files.</div>
        )}
        {changes && changes.files.length > 0 && (
          <ul className="import-items" aria-label="Files that are different">
            {changes.files.map((f) => (
              <li key={f.path}>
                <div className="import-row">
                  <span>{f.path}</span>
                  {changes.other && f.side === 'Both' ? (
                    <select
                      className="field"
                      aria-label={`Version of ${f.path}`}
                      value={picks[f.path] ?? ''}
                      disabled={busy}
                      onChange={(e) => setPicks((p) => ({ ...p, [f.path]: e.target.value as UpdatePick }))}
                    >
                      <option value="" disabled>
                        Pick a version
                      </option>
                      <option value="Other">Other project</option>
                      <option value="This">This project</option>
                    </select>
                  ) : (
                    <span className="hint">{changes.other && f.side !== 'Both' ? SIDES[f.side] : ''}</span>
                  )}
                </div>
                <div className="hint">{STATES[f.state]}</div>
              </li>
            ))}
          </ul>
        )}
        {error && <div className="error-text">{error}</div>}
        <div className="actions">
          <button className="btn primary" disabled={!ready || busy} onClick={start}>
            {busy ? 'Building' : 'Update'}
          </button>
          <button className="btn" disabled={busy} onClick={onClose}>
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
}

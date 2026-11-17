// The Import dialog (spec 6.2, 15.4): it copies a folder of another project into this project.

import { useEffect, useState } from 'react';
import { api, type ImportChoice, type ImportItem, type View } from '../api';

interface Props {
  /** The folder of the other project. */
  other: string;
  onDone: (view: View) => void;
  onClose: () => void;
}

/** The choice of the user is complete: a rename needs a name. */
function complete(choice: ImportChoice | undefined): boolean {
  return choice !== undefined && (choice.kind !== 'Rename' || choice.name.trim() !== '');
}

export function ImportDialog({ other, onDone, onClose }: Props) {
  const [folders, setFolders] = useState<string[]>([]);
  const [folder, setFolder] = useState('');
  const [items, setItems] = useState<ImportItem[]>([]);
  const [choices, setChoices] = useState<Record<string, ImportChoice>>({});
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    api.importFolders(other).then(setFolders).catch((e) => setError(String(e)));
  }, [other]);
  useEffect(() => {
    setItems([]);
    setChoices({});
    setError('');
    if (folder) api.importPlan(other, folder).then(setItems).catch((e) => setError(String(e)));
  }, [other, folder]);

  const start = async () => {
    setBusy(true);
    setError('');
    try {
      onDone(await api.importFolder(other, folder, choices));
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  };
  const choose = (key: string, choice: ImportChoice | undefined) =>
    setChoices((c) => {
      const next = { ...c };
      if (choice) next[key] = choice;
      else delete next[key];
      return next;
    });
  const copies = items.some((i) => i.state.kind !== 'Same');
  const ready = copies && items.every((i) => i.state.kind !== 'Conflict' || complete(choices[i.folder]));
  const name = other.split(/[\\/]/).filter(Boolean).pop() ?? other;
  return (
    <div className="backdrop" onKeyDown={(e) => e.key === 'Escape' && !busy && onClose()}>
      <div className="dialog import" role="dialog" aria-modal="true" aria-labelledby="import-title">
        <h2 id="import-title">Import from {name}</h2>
        <select className="field" aria-label="Folder to import" value={folder} autoFocus disabled={busy} onChange={(e) => setFolder(e.target.value)}>
          <option value="">Select a folder</option>
          {folders.map((f) => (
            <option key={f}>{f}</option>
          ))}
        </select>
        {items.length > 0 && (
          <ul className="import-items" aria-label="Folders that the import copies">
            {items.map((i) => {
              const choice = choices[i.folder];
              const base = i.folder.slice(i.folder.indexOf('/') + 1);
              return (
                <li key={i.folder}>
                  <div className="import-row">
                    <span>{i.folder}</span>
                    {i.state.kind === 'New' && <span className="hint">New</span>}
                    {i.state.kind === 'Same' && <span className="hint">Same files, not copied</span>}
                    {i.state.kind === 'Conflict' && (
                      <select
                        className="field"
                        aria-label={`Choice for ${i.folder}`}
                        value={choice?.kind ?? ''}
                        disabled={busy}
                        onChange={(e) => {
                          const kind = e.target.value;
                          choose(i.folder, kind === 'Rename' ? { kind, name: `${base}-2` } : kind === 'Replace' || kind === 'Keep' ? { kind } : undefined);
                        }}
                      >
                        <option value="">Pick a choice</option>
                        {i.state.replace && <option value="Replace">Replace</option>}
                        {i.state.replace && <option value="Keep">Keep</option>}
                        <option value="Rename">Rename</option>
                      </select>
                    )}
                  </div>
                  {i.state.kind === 'Conflict' && (
                    <div className="hint">
                      This project has {i.state.existing} with other files.
                      {i.users.length > 0 && ` A replace changes ${i.users.join(', ')}.`}
                    </div>
                  )}
                  {choice?.kind === 'Rename' && (
                    <input
                      className="field"
                      aria-label={`New name for ${i.folder}`}
                      value={choice.name}
                      disabled={busy}
                      onChange={(e) => choose(i.folder, { kind: 'Rename', name: e.target.value })}
                    />
                  )}
                </li>
              );
            })}
          </ul>
        )}
        {folder && items.length > 0 && !copies && <div className="hint">This project has these folders already.</div>}
        {error && <div className="error-text">{error}</div>}
        <div className="actions">
          <button className="btn primary" disabled={!ready || busy} onClick={start}>
            {busy ? 'Building' : 'Import'}
          </button>
          <button className="btn" disabled={busy} onClick={onClose}>
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
}

// The New project dialog of the start screen (spec 15.10): a name, a location and Create.

import { useState, type FormEvent } from 'react';
import { api, type View } from '../api';
import { projectFolder, projectNameProblem } from './projectName.ts';

interface Props {
  /** The folder of new projects (13.3), the first location. */
  projectsDir: string;
  onOpen: (view: View) => void;
  onClose: () => void;
}

export function NewProjectDialog({ projectsDir, onOpen, onClose }: Props) {
  const [name, setName] = useState('');
  const [parent, setParent] = useState(projectsDir);
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const clean = name.trim();
  const problem = projectNameProblem(clean);
  const ready = !busy && clean !== '' && !problem && parent.trim() !== '';

  const create = (e: FormEvent) => {
    e.preventDefault();
    if (!ready) return;
    setBusy(true);
    setError('');
    api.newProject(parent.trim(), clean).then(onOpen, (err) => {
      setError(String(err));
      setBusy(false);
    });
  };
  const browse = () => {
    api
      .pickFolder(parent.trim())
      .then((folder) => folder && setParent(folder))
      .catch((err) => setError(String(err)));
  };

  return (
    <div className="backdrop" onKeyDown={(e) => e.key === 'Escape' && !busy && onClose()}>
      <form className="dialog new-project" role="dialog" aria-modal="true" aria-labelledby="new-project-title" onSubmit={create}>
        <h2 id="new-project-title">New project</h2>
        <label className="dialog-field">
          <span className="label">Name</span>
          <input className="field" value={name} onChange={(e) => setName(e.target.value)} placeholder="my-project" spellCheck={false} disabled={busy} autoFocus />
        </label>
        <div className="dialog-field">
          <label className="label" htmlFor="new-project-location">
            Location
          </label>
          <div className="start-row">
            <input id="new-project-location" className="field" value={parent} onChange={(e) => setParent(e.target.value)} spellCheck={false} disabled={busy} />
            <button className="btn" type="button" disabled={busy} onClick={browse}>
              Browse
            </button>
          </div>
        </div>
        {problem ? (
          <div className="error-text">{problem}</div>
        ) : (
          <div className="hint new-project-path">{clean ? `Patina makes the folder ${projectFolder(parent.trim(), clean)}` : 'Type a name for the project folder.'}</div>
        )}
        {error && <div className="error-text">{error}</div>}
        <div className="actions">
          <button className="btn primary" disabled={!ready}>
            {busy ? 'Creating' : 'Create'}
          </button>
          <button className="btn" type="button" disabled={busy} onClick={onClose}>
            Cancel
          </button>
        </div>
      </form>
    </div>
  );
}

// The start screen (spec 15.10): a new project, an open project and the recent projects.

import { useState, type FormEvent } from 'react';
import { AboutDialog, useVersion } from './About';
import { api, type StartInfo, type View } from '../api';
import { Icon } from '../panels/Dock';
import { Confirm } from './Confirm';
import { StartBackdrop } from './StartBackdrop';

type Recent = StartInfo['recent'][number];

interface Props {
  start: StartInfo;
  onOpen: (view: View) => void;
  /** Gives the new recent list after a project goes off the list. */
  onStart: (start: StartInfo) => void;
  onSettings: () => void;
}

export function StartScreen({ start, onOpen, onStart, onSettings }: Props) {
  const [parent, setParent] = useState(start.projectsDir);
  const [name, setName] = useState('');
  const [path, setPath] = useState('');
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const [about, setAbout] = useState(false);
  const [remove, setRemove] = useState<Recent | null>(null);
  const version = useVersion();

  const act = (job: Promise<View>) => {
    setBusy(true);
    setError('');
    job.then(onOpen).catch((e) => setError(String(e))).finally(() => setBusy(false));
  };
  const change = (job: Promise<StartInfo>) => {
    setRemove(null);
    setBusy(true);
    setError('');
    job.then(onStart).catch((e) => setError(String(e))).finally(() => setBusy(false));
  };
  const create = (e: FormEvent) => {
    e.preventDefault();
    act(api.newProject(parent.trim(), name.trim()));
  };
  const open = (e: FormEvent) => {
    e.preventDefault();
    act(api.openProject(path.trim()));
  };
  const browse = (from: string, set: (folder: string) => void) => {
    api
      .pickFolder(from.trim())
      .then((folder) => folder && set(folder))
      .catch((e) => setError(String(e)));
  };

  return (
    <main className="start">
      <StartBackdrop />
      <div className="start-card">
        <div className="hero">
          <h1>Patina</h1>
          <div className="label">{version && `Version ${version}`}</div>
          <div className="hero-actions">
            <button className="btn" onClick={() => setAbout(true)}>
              About
            </button>
            <button className="btn" onClick={onSettings}>
              Settings
            </button>
          </div>
        </div>
        <form className="start-box" onSubmit={create}>
          <h2>New project</h2>
          <div className="start-row">
            <label>
              <span className="label">Folder</span>
              <input className="field" value={parent} onChange={(e) => setParent(e.target.value)} spellCheck={false} />
            </label>
            <button className="btn" type="button" disabled={busy} onClick={() => browse(parent, setParent)}>
              Browse
            </button>
            <label>
              <span className="label">Name</span>
              <input className="field" value={name} onChange={(e) => setName(e.target.value)} spellCheck={false} autoFocus />
            </label>
            <button className="btn primary" disabled={busy || !name.trim() || !parent.trim()}>
              Create
            </button>
          </div>
        </form>
        <form className="start-box" onSubmit={open}>
          <h2>Open project</h2>
          <div className="start-row">
            <label>
              <span className="label">Project folder</span>
              <input className="field" value={path} onChange={(e) => setPath(e.target.value)} spellCheck={false} />
            </label>
            <button className="btn" type="button" disabled={busy} onClick={() => browse(path || parent, setPath)}>
              Browse
            </button>
            <button className="btn" disabled={busy || !path.trim()}>
              Open
            </button>
          </div>
        </form>
        {error && <div className="error-text">{error}</div>}
        <div className="start-box">
          <h2>Recent projects</h2>
          {start.recent.length === 0 && <div className="label">None</div>}
          <div className="recent">
            {start.recent.map((r) => (
              <div key={r.path} className="recent-row">
                <button className={r.exists ? 'item' : 'item empty'} disabled={busy || !r.exists} onClick={() => act(api.openProject(r.path))}>
                  <span>{r.name}</span>
                  <span className="path">{r.path}</span>
                  {!r.exists && <span className="hint">Missing</span>}
                </button>
                <button className="icon-btn recent-remove" aria-label={`Remove ${r.name}`} title="Remove" disabled={busy} onClick={() => setRemove(r)}>
                  <Icon d="m5 5 6 6m0-6-6 6" />
                </button>
              </div>
            ))}
          </div>
        </div>
      </div>
      {about && <AboutDialog onClose={() => setAbout(false)} />}
      {remove && (
        <Confirm
          title={`Remove ${remove.name}?`}
          label={remove.exists ? `Delete project moves the folder ${remove.path} to the Recycle Bin.` : 'The project folder is missing.'}
          answers={[
            ['Remove from list', () => change(api.forgetProject(remove.path))],
            ...(remove.exists ? [['Delete project', () => change(api.deleteProject(remove.path))] as [string, () => void]] : []),
            ['Cancel', () => setRemove(null)],
          ]}
          onCancel={() => setRemove(null)}
        />
      )}
    </main>
  );
}

// The start screen (spec 15.10): a new project, an open project and the recent projects.

import { useState, type FormEvent } from 'react';
import { AboutDialog, useVersion } from './About';
import { api, type StartInfo, type View } from './api';

interface Props {
  start: StartInfo;
  onOpen: (view: View) => void;
  onSettings: () => void;
}

export function StartScreen({ start, onOpen, onSettings }: Props) {
  const [parent, setParent] = useState(start.projectsDir);
  const [name, setName] = useState('');
  const [path, setPath] = useState('');
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const [about, setAbout] = useState(false);
  const version = useVersion();

  const act = (job: Promise<View>) => {
    setBusy(true);
    setError('');
    job.then(onOpen).catch((e) => setError(String(e))).finally(() => setBusy(false));
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
      <div className="start-card">
        <img className="splash" src="/brand/splash.png" alt="" />
        <div className="settings-head">
          <h1 className="brand-title">
            Patina <span className="label">{version && `Version ${version}`}</span>
          </h1>
          <div className="start-row">
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
              <button
                key={r.path}
                className={r.exists ? 'item' : 'item empty'}
                disabled={busy || !r.exists}
                onClick={() => act(api.openProject(r.path))}
              >
                <span>{r.name}</span>
                <span className="path">{r.path}</span>
                {!r.exists && <span className="hint">Missing</span>}
              </button>
            ))}
          </div>
        </div>
      </div>
      {about && <AboutDialog onClose={() => setAbout(false)} />}
    </main>
  );
}

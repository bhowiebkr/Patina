// The start screen (spec 15.10): New project, Open project, the included workbenches and the recent projects.

import { useEffect, useRef, useState, type KeyboardEvent } from 'react';
import { AboutDialog, useVersion } from './About';
import { api, type Settings, type StartInfo, type View } from '../api';
import { Icon } from '../panels/Dock';
import { Confirm } from './Confirm';
import { NewProjectDialog } from './NewProjectDialog';
import { StartBackdrop } from './StartBackdrop';
import { InstallProgressBar, useToolchainInstall } from './ToolchainInstall';

type Recent = StartInfo['recent'][number];

interface Props {
  start: StartInfo;
  onOpen: (view: View) => void;
  /** Gives the new recent list after a project goes off the list. */
  onStart: (start: StartInfo) => void;
  onSettings: () => void;
}

export function StartScreen({ start, onOpen, onStart, onSettings }: Props) {
  const [creating, setCreating] = useState(false);
  const [filter, setFilter] = useState('');
  const [error, setError] = useState('');
  const [busy, setBusy] = useState('');
  const [about, setAbout] = useState(false);
  const [remove, setRemove] = useState<Recent | null>(null);
  const list = useRef<HTMLDivElement>(null);
  const version = useVersion();
  // The first start: Patina has no toolchain of its own, so a card offers the install (13.2).
  const [settings, setSettings] = useState<Settings | null>(null);
  const read = () => api.settings().then(setSettings, () => {});
  const { install, start: startInstall } = useToolchainInstall(read, setError);
  useEffect(() => {
    read();
  }, []);
  const installing = install ? !install.done : !!settings?.installing;

  const act = (job: Promise<View>) => {
    setBusy('Opening the project...');
    setError('');
    job.then(onOpen).catch((e) => setError(String(e))).finally(() => setBusy(''));
  };
  const change = (job: Promise<StartInfo>) => {
    setRemove(null);
    setBusy('Changing the list...');
    setError('');
    job.then(onStart).catch((e) => setError(String(e))).finally(() => setBusy(''));
  };
  // Open project opens the folder dialog, and the selected folder opens at once (15.10).
  const openFolder = () => {
    setError('');
    api
      .pickFolder(start.projectsDir)
      .then((folder) => folder && act(api.openProject(folder)))
      .catch((e) => setError(String(e)));
  };
  const pick = (r: Recent) => (r.exists ? act(api.openProject(r.path)) : setRemove(r));

  // Ctrl+N and Ctrl+O work only on the start screen with no dialog open.
  const keys = useRef({ ready: false, openFolder });
  keys.current = { ready: !busy && !creating && !about && !remove, openFolder };
  useEffect(() => {
    const onKey = (e: globalThis.KeyboardEvent) => {
      if (!e.ctrlKey || e.shiftKey || e.altKey || !keys.current.ready) return;
      const key = e.key.toLowerCase();
      if (key !== 'n' && key !== 'o') return;
      e.preventDefault();
      if (key === 'n') setCreating(true);
      else keys.current.openFolder();
    };
    addEventListener('keydown', onKey);
    return () => removeEventListener('keydown', onKey);
  }, []);

  const words = filter.trim().toLowerCase();
  const shown = words ? start.recent.filter((r) => `${r.name}\n${r.path}`.toLowerCase().includes(words)) : start.recent;
  const rows = () => [...(list.current?.querySelectorAll<HTMLButtonElement>('button.item') ?? [])];
  const onFilterKey = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter' && shown.length > 0 && !busy) pick(shown[0]);
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      rows()[0]?.focus();
    }
  };
  const onListKey = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp') return;
    e.preventDefault();
    const all = rows();
    const at = all.indexOf(document.activeElement as HTMLButtonElement);
    const next = at + (e.key === 'ArrowDown' ? 1 : -1);
    if (next >= 0) all[Math.min(next, all.length - 1)]?.focus();
    else document.querySelector<HTMLInputElement>('.recent-filter')?.focus();
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
        {settings && !settings.ownToolchain && (
          <section className="start-box first-start" aria-label="Toolchain">
            <h2>Install the toolchain</h2>
            <div className="label">
              Patina builds each node with Rust. <b>Install toolchain</b> downloads Rust and Zig, about 265 MB, into the folder of Patina. It needs no admin rights.
              {settings.rust && ` Until then, Patina uses the Rust of the system (${settings.rust}).`}
            </div>
            <div className="label">
              Later, <b>Settings</b> at the top right has the toolchain, the update and the folders.
            </div>
            <div className="first-start-actions">
              <button className="btn primary" disabled={installing} onClick={startInstall(api.installToolchain)}>
                {install?.error ? 'Retry' : 'Install toolchain'}
              </button>
              <button className="btn" onClick={onSettings}>
                Settings
              </button>
            </div>
            <InstallProgressBar install={install} />
          </section>
        )}
        <div className="start-actions">
          <button className="start-action" title="New project (Ctrl+N)" disabled={!!busy} onClick={() => setCreating(true)}>
            <Icon d="M8 3v10M3 8h10" />
            <span className="start-action-title">New project</span>
            <span className="start-action-text">Make an empty project in a new folder.</span>
          </button>
          <button className="start-action" title="Open project (Ctrl+O)" disabled={!!busy} onClick={openFolder}>
            <Icon d="M2 4.5h4l1.5 1.5H14v6.5H2z" />
            <span className="start-action-title">Open project</span>
            <span className="start-action-text">Select a project folder and open it.</span>
          </button>
        </div>
        {busy && <div className="label start-busy">{busy}</div>}
        {error && <div className="error-text">{error}</div>}
        {start.workbenches.length > 0 && (
          <section className="start-box" aria-label="Workbenches">
            <h2>Workbenches</h2>
            <div className="workbench-list">
              {start.workbenches.map((w) => (
                <div key={w.path} className="workbench" title={w.path}>
                  <span className="name">{w.name}</span>
                  <button className="btn" aria-label={`Open ${w.name}`} disabled={!!busy} onClick={() => act(api.openProject(w.path))}>
                    Open
                  </button>
                </div>
              ))}
            </div>
          </section>
        )}
        <section className="start-box" aria-label="Recent projects">
          <div className="recent-top">
            <h2>Recent projects</h2>
            {start.recent.length > 0 && (
              <input
                className="field recent-filter"
                aria-label="Filter recent projects"
                placeholder="Filter by name or path"
                value={filter}
                onChange={(e) => setFilter(e.target.value)}
                onKeyDown={onFilterKey}
                spellCheck={false}
              />
            )}
          </div>
          {start.recent.length === 0 ? (
            <div className="label">
              No recent projects yet. Click <b>New project</b> to start a project, or <b>Open project</b> to open a project folder.
            </div>
          ) : shown.length === 0 ? (
            <div className="label">No project matches the filter.</div>
          ) : (
            <>
              <div className="recent-head" aria-hidden="true">
                <span>Name</span>
                <span>Path</span>
              </div>
              <div className="recent" ref={list} onKeyDown={onListKey}>
                {shown.map((r) => (
                  <div key={r.path} className="recent-row">
                    <button className={r.exists ? 'item' : 'item empty'} disabled={!!busy} onClick={() => pick(r)}>
                      <span className="name" title={r.name}>
                        {r.name}
                      </span>
                      <span className="path" title={r.path}>
                        {r.path}
                      </span>
                      {!r.exists && <span className="hint">Missing</span>}
                    </button>
                    <button className="icon-btn recent-remove" aria-label={`Remove ${r.name}`} title="Remove" disabled={!!busy} onClick={() => setRemove(r)}>
                      <Icon d="m5 5 6 6m0-6-6 6" />
                    </button>
                  </div>
                ))}
              </div>
            </>
          )}
        </section>
      </div>
      {creating && <NewProjectDialog projectsDir={start.projectsDir} onOpen={onOpen} onClose={() => setCreating(false)} />}
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

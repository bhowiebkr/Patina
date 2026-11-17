// The settings screen (spec 15.10): the toolchain, the folders, the libraries and the trusted projects.

import { useEffect, useState } from 'react';
import { api, onToolchain, type InstallProgress, type Settings } from '../api';
import { useEvent } from '../useEvent';
import { StartBackdrop } from './StartBackdrop';
import { LibrarySettings } from './LibrarySettings';

/** Back or Esc returns to the start screen. */
export function SettingsScreen({ onBack }: { onBack: () => void }) {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [error, setError] = useState('');
  const [install, setInstall] = useState<InstallProgress | null>(null);
  const read = () => api.settings().then(setSettings, (e) => setError(String(e)));
  useEffect(() => {
    read();
  }, []);
  useEvent(onToolchain, (progress: InstallProgress) => {
    setInstall(progress);
    if (progress.done && !progress.error) read();
  });
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === 'Escape' && onBack();
    addEventListener('keydown', onKey);
    return () => removeEventListener('keydown', onKey);
  }, [onBack]);

  const untrust = (path: string) => {
    setError('');
    api.untrustProject(path).then(
      (trusted) => setSettings((s) => s && { ...s, trusted }),
      (e) => setError(String(e)),
    );
  };
  const start = (call: () => Promise<void>) => () => {
    setError('');
    setInstall({ step: 'Start', done: false, error: null });
    call().catch((e) => {
      setInstall(null);
      setError(String(e));
    });
  };
  const s = settings;
  const installing = install ? !install.done : !!s?.installing;
  return (
    <main className="start" aria-label="Settings">
      <StartBackdrop />
      <div className="start-card">
        <div className="settings-head">
          <h1>Settings</h1>
          <button className="btn" autoFocus onClick={onBack}>
            Back
          </button>
        </div>
        {error && <div className="error-text">{error}</div>}
        <section className="start-box" aria-label="Toolchain">
          <h2>Toolchain</h2>
          {!s && <div className="label">Reading</div>}
          {s && (
            <dl className="settings-list">
              {s.rust && (
                <>
                  <dt>Rust</dt>
                  <dd>{s.rust}</dd>
                  <dt>Cargo</dt>
                  <dd>{s.cargo}</dd>
                </>
              )}
              <dt>Toolchain folder</dt>
              <dd>
                {s.toolchainDir}
                {!s.ownToolchain && <span className="label"> (empty: Patina uses the Rust of the system)</span>}
              </dd>
            </dl>
          )}
          {s && !s.ownToolchain && (
            <div className="settings-row">
              <span className="label">
                {installing ? (install?.step ?? 'Patina installs the toolchain') : 'Patina downloads Rust and Zig, about 265 MB, into the toolchain folder.'}
              </span>
              <button className="btn" disabled={installing} onClick={start(api.installToolchain)}>
                {install?.error ? 'Retry' : 'Install toolchain'}
              </button>
            </div>
          )}
          {s?.ownToolchain && (
            <div className="settings-row">
              <span className="label">
                {installing ? (install?.step ?? 'Patina updates the toolchain') : 'Patina downloads the newest stable Rust into the toolchain folder. Zig and the crate cache stay.'}
              </span>
              <button className="btn" disabled={installing} onClick={start(api.updateToolchain)}>
                {install?.error ? 'Retry' : 'Update toolchain'}
              </button>
            </div>
          )}
          {install?.error && (
            <div className="error-text">
              {install.step} failed: {install.error}
            </div>
          )}
          {s?.toolchainProblem && <div className="error-text">{s.toolchainProblem}</div>}
        </section>
        <section className="start-box" aria-label="Folders">
          <h2>Folders</h2>
          {s && (
            <dl className="settings-list">
              <dt>Data</dt>
              <dd>
                {s.dataDir}
                {s.portable && <span className="label"> (portable)</span>}
              </dd>
              <dt>New projects</dt>
              <dd>{s.projectsDir}</dd>
            </dl>
          )}
        </section>
        <LibrarySettings libraries={s?.libraries} projectsDir={s?.projectsDir ?? ''} onChange={read} onError={setError} />
        <section className="start-box" aria-label="Trusted projects">
          <h2>Trusted projects</h2>
          <div className="label">Patina builds and runs the code of these folders. Remove a folder, and Patina asks again when the project opens.</div>
          {s?.trusted.length === 0 && <div className="label">None</div>}
          {s?.trusted.map((path) => (
            <div key={path} className="settings-row">
              <span className="path">{path}</span>
              <button className="btn" aria-label={`Remove ${path}`} onClick={() => untrust(path)}>
                Remove
              </button>
            </div>
          ))}
        </section>
      </div>
    </main>
  );
}

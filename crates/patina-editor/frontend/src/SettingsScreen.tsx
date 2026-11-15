// The settings screen (spec 15.10): the toolchain, the folders and the trusted projects.

import { useEffect, useState } from 'react';
import { api, type Settings } from './api';

/** Back or Esc returns to the start screen. */
export function SettingsScreen({ onBack }: { onBack: () => void }) {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [error, setError] = useState('');
  useEffect(() => {
    api.settings().then(setSettings, (e) => setError(String(e)));
  }, []);
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
  const s = settings;
  return (
    <main className="start" aria-label="Settings">
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

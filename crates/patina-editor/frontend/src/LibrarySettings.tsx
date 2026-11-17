// The libraries part of the settings (spec 6.2, 15.10): local Patina projects and copies from Git.

import { useState } from 'react';
import { api, type Checked, type Library } from './api';

/** `onChange` reads the settings again, because a library change also changes the trusted list. */
export function LibrarySettings({ libraries, projectsDir, onChange, onError }: { libraries: Library[] | undefined; projectsDir: string; onChange: () => void; onError: (text: string) => void }) {
  const [url, setUrl] = useState('');
  const [tag, setTag] = useState('');
  const [busy, setBusy] = useState('');
  const [checks, setChecks] = useState<Checked[]>([]);
  const run = async (step: string, job: () => Promise<unknown>) => {
    onError('');
    setBusy(step);
    try {
      await job();
      onChange();
    } catch (e) {
      onError(String(e));
    } finally {
      setBusy('');
    }
  };
  const addLocal = () =>
    run('', async () => {
      const path = await api.pickFolder(projectsDir);
      if (path) await api.addLibrary(path);
    });
  const addGit = () =>
    run(`Patina downloads ${tag.trim() || 'the newest tag'} of ${url.trim()}`, async () => {
      await api.addGitLibrary(url, tag);
      setUrl('');
      setTag('');
    });
  const check = () => run('Patina reads the tags of the libraries from Git', async () => setChecks(await api.checkLibraries()));
  const update = (path: string, newer: string) =>
    run(`Patina downloads ${newer}`, async () => {
      await api.updateLibrary(path, newer);
      setChecks((list) => list.filter((c) => c.path !== path));
    });
  const disabled = !libraries || busy !== '';
  return (
    <section className="start-box" aria-label="Libraries">
      <h2>Libraries</h2>
      <div className="label">The node palette shows the nodes and the subgraphs of these Patina projects. Patina trusts each library, because it builds their code.</div>
      {libraries?.length === 0 && <div className="label">None</div>}
      {libraries?.map(({ path, pin }) => {
        const c = checks.find((c) => c.path === path);
        return (
          <div key={path}>
            <div className="settings-row">
              <span className="path" title={pin ? `${path}\ncommit ${pin.commit}` : undefined}>
                {pin ? `${pin.url} ${pin.tag}` : path}
              </span>
              <span>
                {c?.newer && (
                  <button className="btn" disabled={disabled} onClick={() => update(path, c.newer!)}>
                    Update to {c.newer}
                  </button>
                )}
                <button className="btn" disabled={disabled} aria-label={`Remove library ${path}`} onClick={() => run('', () => api.removeLibrary(path))}>
                  Remove
                </button>
              </span>
            </div>
            {c?.moved && <div className="error-text">The tag {pin?.tag} points to another commit now. Patina keeps the old copy.</div>}
            {c?.error && <div className="error-text">{c.error}</div>}
          </div>
        );
      })}
      <div className="settings-row">
        <span className="label">{busy}</span>
        <span>
          {libraries?.some((l) => l.pin) && (
            <button className="btn" disabled={disabled} onClick={check}>
              Check for updates
            </button>
          )}
          <button className="btn" disabled={disabled} onClick={addLocal}>
            Add library
          </button>
        </span>
      </div>
      <div className="settings-row">
        <input className="field" aria-label="Git URL" placeholder="Git URL: https://github.com/owner/name" value={url} disabled={disabled} onChange={(e) => setUrl(e.target.value)} />
        <input className="field" aria-label="Tag" placeholder="Tag (empty: newest)" value={tag} disabled={disabled} onChange={(e) => setTag(e.target.value)} />
        <button className="btn" disabled={disabled || !url.trim()} onClick={addGit}>
          Add from Git
        </button>
      </div>
    </section>
  );
}

// The Help menu and the About dialog (spec 15.3). The artwork has no text: the page writes the
// name, the version and the license.

import { useEffect, useRef, useState } from 'react';
import { getVersion } from '@tauri-apps/api/app';
import { api } from '../api';
import { Icon, Menu, below } from '../panels/Dock';

/** The version of the editor, or an empty text until it is known. */
export function useVersion() {
  const [version, setVersion] = useState('');
  useEffect(() => {
    getVersion().then(setVersion, () => setVersion(''));
  }, []);
  return version;
}

/** The About dialog: the banner artwork, the name, the version and the license. Patina reads
 * the newest release from GitHub only when the person clicks **Check for updates**. */
export function AboutDialog({ onClose }: { onClose: () => void }) {
  const version = useVersion();
  const [update, setUpdate] = useState('');
  const check = () => {
    setUpdate('Checking for updates...');
    api.checkUpdate().then(
      (release) =>
        setUpdate(
          release === null
            ? 'Patina has no release on GitHub yet.'
            : release.newer
              ? `Patina ${release.version} is available at ${release.url}`
              : 'This is the newest version.',
        ),
      (e) => setUpdate(`Cannot check for updates: ${e}`),
    );
  };
  return (
    <div className="backdrop" onKeyDown={(e) => e.key === 'Escape' && onClose()}>
      <div className="dialog about" role="dialog" aria-modal="true" aria-labelledby="about-title">
        <div className="about-art">
          <h2 id="about-title">Patina</h2>
        </div>
        <div className="label">{version && `Version ${version}`}</div>
        <div className="label">MIT or Apache-2.0</div>
        <div className="label update" aria-live="polite">
          {update}
        </div>
        <div className="actions">
          <button className="btn" disabled={update === 'Checking for updates...'} onClick={check}>
            Check for updates
          </button>
          <button className="btn primary" autoFocus onClick={onClose}>
            Close
          </button>
        </div>
      </div>
    </div>
  );
}

/** The Help menu of the top bar, with **About Patina**. */
export function HelpMenu() {
  const [at, setAt] = useState<{ x: number; y: number } | null>(null);
  const [about, setAbout] = useState(false);
  const button = useRef<HTMLButtonElement>(null);
  const closeAbout = () => {
    setAbout(false);
    button.current?.focus();
  };
  return (
    <>
      <button ref={button} className="btn" aria-haspopup="menu" aria-expanded={at !== null} onClick={(e) => setAt(below(e.currentTarget))}>
        Help
        <Icon d="m4 6 4 4 4-4" />
      </button>
      {at && <Menu {...at} label="Help" items={[{ label: 'About Patina', run: () => setAbout(true) }]} onClose={() => setAt(null)} />}
      {about && <AboutDialog onClose={closeAbout} />}
    </>
  );
}

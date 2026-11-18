// The progress of Install toolchain and Update toolchain (13.2), on the start screen and in Settings.

import { useState } from 'react';
import { onToolchain, type InstallProgress } from '../api';
import { useEvent } from '../useEvent';

/** The state of the install. `onDone` reads the settings again after a good install. */
export function useToolchainInstall(onDone: () => void, onError: (text: string) => void) {
  const [install, setInstall] = useState<InstallProgress | null>(null);
  useEvent(onToolchain, (progress: InstallProgress) => {
    setInstall(progress);
    if (progress.done && !progress.error) onDone();
  });
  const start = (call: () => Promise<void>) => () => {
    onError('');
    setInstall({ step: 'Start', fraction: 0, done: false, error: null });
    call().catch((e) => {
      setInstall(null);
      onError(String(e));
    });
  };
  return { install, start };
}

/** The progress bar and the step of a running install, or the error of the last install. */
export function InstallProgressBar({ install }: { install: InstallProgress | null }) {
  if (install?.error) {
    return (
      <div className="error-text">
        {install.step} failed: {install.error}
      </div>
    );
  }
  if (!install || install.done) return null;
  const percent = Math.floor(install.fraction * 100);
  return (
    <div className="install-progress">
      <div className="gui-progress" role="progressbar" aria-label="Toolchain install" aria-valuemin={0} aria-valuemax={100} aria-valuenow={percent}>
        <div style={{ width: `${install.fraction * 100}%` }} />
      </div>
      <span className="label">
        {install.step} - {percent} %
      </span>
    </div>
  );
}

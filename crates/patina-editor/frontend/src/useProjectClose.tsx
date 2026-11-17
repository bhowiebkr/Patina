// Close and quit with unsaved changes, and the answer to a recovery file (15.10).
import { useCallback, useState, type RefObject } from 'react';
import { api, onCloseRequested, type View } from './api';
import { closeCode, restoreCode } from './code';
import { Confirm } from './Confirm';
import { endLayout, flushLayout } from './Dock';
import { useEvent } from './useEvent';

export function useProjectClose(p: {
  view: View | null;
  dirty: boolean;
  serial: RefObject<number>;
  show: (v: View) => void;
  setView: (v: View | null) => void;
  saveAll: () => Promise<boolean>;
  refreshStart: () => Promise<unknown>;
  setError: (e: string) => void;
}) {
  const { view, dirty, serial, show, setView, saveAll, refreshStart, setError } = p;
  const [ask, setAsk] = useState<'close' | 'quit' | null>(null);
  useEvent(onCloseRequested, (ask) => (ask ? setAsk('quit') : flushLayout().then(() => api.quit())));

  const closeProject = useCallback(() => {
    endLayout()
      .then(() => api.closeProject())
      .then(() => {
        setView(null);
        closeCode();
        serial.current = -1;
        return refreshStart();
      })
      .catch((e) => setError(String(e)));
  }, [refreshStart, setView, serial, setError]);

  // The restored code goes into the Code tab as unsaved text.
  const recover = (restore: boolean) =>
    api
      .recover(restore)
      .then(({ view: v, code }) => {
        show(v);
        return restoreCode(code);
      })
      .then((e) => e && setError(e))
      .catch((e) => setError(String(e)));

  const answer = (choice: 'save' | 'discard' | 'cancel') => {
    const what = ask;
    setAsk(null);
    if (choice === 'cancel' || !what) return;
    const go = () => (what === 'quit' ? flushLayout().then(() => api.quit()) : closeProject());
    if (choice === 'save') saveAll().then((ok) => ok && go());
    else go();
  };

  const close = () => (dirty ? setAsk('close') : closeProject());
  const dialogs = view && (
    <>
      {ask && (
        <Confirm
          title={`Save the changes to ${view.name}?`}
          label="If you do not save, the changes are lost."
          answers={[
            ['Save', () => answer('save')],
            ['Discard', () => answer('discard')],
            ['Cancel', () => answer('cancel')],
          ]}
          onCancel={() => answer('cancel')}
        />
      )}
      {view.recovery && view.trusted && (
        <Confirm
          title={`Restore the unsaved changes to ${view.name}?`}
          label="Patina stopped before you saved the changes."
          answers={[
            ['Restore', () => recover(true)],
            ['Discard', () => recover(false)],
          ]}
        />
      )}
    </>
  );
  return { close, dialogs };
}

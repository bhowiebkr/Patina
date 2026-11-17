// The actions of the Project panel (15.2): new folders, crates, import, update and package. The
// Import and Update dialogs show here.

import { useCallback, useState } from 'react';
import { api, type View } from './api';
import { ImportDialog } from './ImportDialog';
import type { Line } from './Lines';
import type { Group, Inspect } from './ProjectPanel';
import { UpdateDialog } from './UpdateDialog';

interface Host {
  show: (v: View) => void;
  markStale: (v: View) => void;
  log: (lines: Line[]) => void;
  setInspect: (i: Inspect) => void;
  setError: (e: string) => void;
}

export function useProjectActions({ show, markStale, log, setInspect, setError }: Host) {
  const [importing, setImporting] = useState<string | null>(null);
  const [updating, setUpdating] = useState<{ other: string; folder: string } | null>(null);

  /** Makes a code folder and shows its code in the Inspector (15.2). */
  const newFolder = useCallback(
    async (group: Group, name: string) => {
      const jobs = { nodes: api.newNode, widgets: api.newWidget, views: api.newView, connections: api.newConnection, types: api.newType };
      show(await jobs[group](name));
      setInspect(group === 'nodes' ? { kind: 'type', name } : { kind: 'folder', folder: `${group}/${name}` });
    },
    [show, setInspect],
  );

  const pickImport = useCallback(
    () =>
      api
        .pickFolder('')
        .then((folder) => folder && setImporting(folder))
        .catch((e) => setError(String(e))),
    [setError],
  );

  const pickUpdate = useCallback(
    (folder: string) =>
      api
        .updateSource(folder)
        .then((other) => other ?? api.pickFolder(''))
        .then((other) => other && setUpdating({ other, folder }))
        .catch((e) => setError(String(e))),
    [setError],
  );

  const makePackage = useCallback(async () => {
    try {
      const parent = await api.pickFolder('');
      if (!parent) return;
      log([{ text: 'Making the package', kind: 'info' }]);
      const made = await api.makePackage(parent);
      log([...made.warnings.map((w): Line => ({ text: w, kind: 'warn' })), { text: `Made the package ${made.out}`, kind: 'info' }]);
    } catch (e) {
      setError(String(e));
    }
  }, [log, setError]);

  /** Adds a crate to a code folder with `cargo add`. Undo removes it (15.4). */
  const addCrate = useCallback(async (folder: string, spec: string) => show(await api.addCrate(folder, spec)), [show]);

  const done = (close: () => void) => (v: View) => {
    close();
    show(v);
    markStale(v);
  };
  const dialogs = (
    <>
      {importing && <ImportDialog other={importing} onDone={done(() => setImporting(null))} onClose={() => setImporting(null)} />}
      {updating && <UpdateDialog {...updating} onDone={done(() => setUpdating(null))} onClose={() => setUpdating(null)} />}
    </>
  );

  return {
    newFolder,
    pickImport,
    pickUpdate,
    makePackage,
    addCrate,
    dialogs,
  };
}

// The edits of the open project: the graph edits, undo, redo, save, copy and paste (15.10, 15.11).
import { useCallback, type RefObject } from 'react';
import { api, newId, onEngine, paramEdit, type DragLink, type Edit, type EngineEvent, type LibraryNode, type View } from '../api';
import { clipboard } from './clip';
import { saveCode, undoRedo } from '../code/codeState';
import type { Line } from '../run/Lines';
import { useEvent } from '../useEvent';

/** True when the edit changes only the drawing, so the result of the run stays current. */
function onlyGeometry(edit: Edit): boolean {
  return edit.kind === 'MoveNode' || edit.kind === 'MovePin' || edit.kind === 'SetRoute' || (edit.kind === 'Group' && edit.edits.every(onlyGeometry));
}

export function useGraphEdits(p: {
  serial: RefObject<number>;
  current: RefObject<View | null>;
  show: (v: View) => void;
  markStale: (v: View) => void;
  log: (lines: Line[]) => void;
  setError: (e: string) => void;
  setFocus: (focus: { ids: string[]; n: number }) => void;
}) {
  const { serial, current, show, markStale, log, setError, setFocus } = p;
  const act = useCallback(
    (job: Promise<View>, after?: (v: View) => void) =>
      job
        .then((v) => {
          show(v);
          after?.(v);
        })
        .catch((e) => setError(String(e))),
    [show, setError],
  );

  const edit = useCallback((e: Edit) => act(api.edit(e), onlyGeometry(e) ? undefined : markStale), [act, markStale]);
  // A node GUI writes its own parameters through the edit path, so undo removes them (4.3).
  useEvent(onEngine, (e: EngineEvent) => {
    if (e.serial !== serial.current || e.state !== 'gui' || e.params.length === 0 || !current.current) return;
    const graph = current.current.graph;
    const change = paramEdit(graph, e.params);
    if (change) edit(change);
    // The inner nodes of a use are read-only (15.3), so their writes do not apply.
    else if (e.params.some((p) => !graph.node?.some((n) => n.id === p.id))) setError('A node in a subgraph use can not change its parameters. Open the subgraph and use the Inspector.');
  });
  const undo = useCallback(() => act(undoRedo(api.undo), markStale), [act, markStale]);
  const redo = useCallback(() => act(undoRedo(api.redo), markStale), [act, markStale]);
  // Ctrl+S saves all: one SetCode edit for each changed code file, then the graph (15.10).
  const saveAll = useCallback(async () => {
    try {
      const coded = await saveCode();
      if (coded) {
        show(coded);
        markStale(coded);
      }
      show(await api.save());
      return true;
    } catch (e) {
      setError(String(e));
      return false;
    }
  }, [show, markStale, setError]);

  // The subgraph runs alone. Each bool public input starts off, and a click switches it (15.9).
  // It reads the ref, because a click can come before the render of the last edit.
  const startTest = useCallback(() => {
    const v = current.current;
    const info = v?.nodes.find((n) => n.subgraph && n.name === v.graphName);
    act(api.test(Object.fromEntries((info?.inputs ?? []).filter((p) => p.ty === 'bool').map((p) => [p.key, 'false']))));
  }, [act, current]);
  const switchInput = useCallback(
    (key: string) => {
      const inputs = current.current?.test;
      if (inputs) act(api.test({ ...inputs, [key]: inputs[key] === 'true' ? 'false' : 'true' }));
    },
    [act, current],
  );

  // Ctrl+G moves the selected nodes into a new subgraph (6.1).
  const collapse = useCallback(
    async (name: string, ids: string[]) => {
      const v = await api.collapse(name, ids, newId());
      show(v);
      markStale(v);
    },
    [show, markStale],
  );
  const expandUses = useCallback((ids: string[]) => act(api.expand(ids), markStale), [act, markStale]);

  // Copy and paste go through the clipboard, so they also work between two editors (15.11).
  const copyNodes = useCallback(
    async (ids: string[]) => {
      try {
        await clipboard.write(await api.copyClip(ids));
        return true;
      } catch (e) {
        setError(String(e));
        return false;
      }
    },
    [setError],
  );
  const paste = useCallback(
    async (at: [number, number], library?: LibraryNode, link?: DragLink) => {
      try {
        const p = await (library ? api.pasteLibrary(library, at, link) : api.pasteClip(await clipboard.read(), at));
        show(p.view);
        markStale(p.view);
        setFocus({ ids: p.ids, n: 0 });
        if (p.warnings.length > 0) log(p.warnings.map((text): Line => ({ text, kind: 'warn' })));
      } catch (e) {
        setError(String(e));
      }
    },
    [show, markStale, log, setError, setFocus],
  );
  return { act, edit, undo, redo, saveAll, startTest, switchInput, collapse, expandUses, copyNodes, paste };
}

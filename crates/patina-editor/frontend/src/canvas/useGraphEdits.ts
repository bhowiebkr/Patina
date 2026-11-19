// The edits of the open project: the graph edits, undo, redo, save, copy and paste (15.10, 15.11).
import { useCallback, useEffect, useRef, type RefObject } from 'react';
import { api, guiGesture, localId, newId, onEngine, paramEdit, type DragLink, type Edit, type EngineEvent, type LibraryNode, type View } from '../api';
import { clipboard } from './clip';
import { saveCode, undoRedo } from '../code/codeState';
import type { Line } from '../run/Lines';
import { useEvent } from '../useEvent';

/** True when the edit changes only the drawing, so the result of the run stays current. */
function onlyGeometry(edit: Edit): boolean {
  const visual = ['MoveNode', 'MovePin', 'SetRoute', 'SetGroup', 'RemoveGroup', 'SetNote', 'RemoveNote'];
  return visual.includes(edit.kind) || (edit.kind === 'Group' && edit.edits.every(onlyGeometry));
}

export function useGraphEdits(p: {
  serial: RefObject<number>;
  /** The use path of the open graph (6.4). The GUI writes of its nodes have IDs in it. */
  scope: RefObject<string>;
  current: RefObject<View | null>;
  show: (v: View) => void;
  markStale: (v: View) => void;
  log: (lines: Line[]) => void;
  setError: (e: string) => void;
  setFocus: (focus: { ids: string[]; n: number }) => void;
}) {
  const { serial, scope, current, show, markStale, log, setError, setFocus } = p;
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
  // The writes go out in order, so the last write of a drag stays.
  const writes = useRef(Promise.resolve());
  useEvent(onEngine, (e: EngineEvent) => {
    if (e.serial !== serial.current || e.state !== 'gui' || e.params.length === 0 || !current.current) return;
    const graph = current.current.graph;
    // A write in a subgraph that is open through a use, or that runs alone, changes the open subgraph (4.3).
    const change = paramEdit(graph, e.params, scope.current);
    const gesture = guiGesture();
    if (change) writes.current = writes.current.then(() => act(api.edit(change, gesture), markStale));
    // A node of a subgraph inside the open graph is in another file (15.3), so its writes do not apply.
    else if (e.params.some((p) => !graph.node?.some((n) => n.id === localId(scope.current, p.id)))) setError('The node is in a subgraph inside this graph. Open that subgraph to change the node.');
  });
  // A change of a code or graph file from outside the editor checks and builds again (15.10).
  useEffect(() => {
    let pending = false;
    const refresh = () => {
      if (pending || !current.current) return;
      pending = true;
      api
        .refresh()
        .then((v) => v && (show(v), markStale(v)))
        .catch((e) => setError(String(e)))
        .finally(() => (pending = false));
    };
    const timer = setInterval(refresh, 2000);
    window.addEventListener('focus', refresh);
    return () => {
      clearInterval(timer);
      window.removeEventListener('focus', refresh);
    };
  }, [current, show, markStale, setError]);
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
        // The canvas IDs of the groups and the notes have their kind in front (15.12).
        const marks = new Map([...(p.view.graph.group ?? []).map((g): [string, string] => [g.id, `group:${g.id}`]), ...(p.view.graph.note ?? []).map((n): [string, string] => [n.id, `note:${n.id}`])]);
        setFocus({ ids: p.ids.map((id) => marks.get(id) ?? id), n: 0 });
        if (p.warnings.length > 0) log(p.warnings.map((text): Line => ({ text, kind: 'warn' })));
      } catch (e) {
        setError(String(e));
      }
    },
    [show, markStale, log, setError, setFocus],
  );
  return { act, edit, undo, redo, saveAll, startTest, switchInput, collapse, expandUses, copyNodes, paste };
}

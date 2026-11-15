// The Code tab of the Inspector (spec 15.4, 15.10): the files of a node folder in Monaco, the
// build state of the saved code and the compile errors of the node (15.5).

import { useEffect, useRef, useState, useSyncExternalStore } from 'react';
import { api, errorText, seconds, type Diagnostic } from './api';
import { monaco, openFile, subscribe, unsavedFiles } from './code';
import { nodeErrors, type RunState } from './NodeView';

/** A request to show a line of a file. A new `n` shows the line again. */
export interface Reveal {
  path: string;
  line: number;
  col: number;
  n: number;
}

export function revealOf(e: Diagnostic, n: number): Reveal | null {
  return e.file ? { path: e.file, line: e.start[0], col: e.start[1], n } : null;
}

/** The build state of the saved code, and true when it is an error. */
export function buildState(run: RunState): [string, boolean] {
  if (run.phase === 'building') return ['Building', false];
  if (run.phase === 'ran' && run.errors.length > 0) return ['Running the last build that compiled', true];
  if (run.phase === 'failed') return ['Error', true];
  if (run.phase === 'ran' && run.lastBuildMs !== null) return [`Built in ${seconds(run.lastBuildMs)}`, false];
  return ['Not built', false];
}

interface Props {
  /** The node folder, for example `number` for `nodes/number`. */
  name: string;
  run: RunState;
  readOnly: boolean;
  reveal: Reveal | null;
  onSave: () => void;
}

export function CodeTab({ name, run, readOnly, reveal, onSave }: Props) {
  const [files, setFiles] = useState<{ name: string; list: string[] } | null>(null);
  const [path, setPath] = useState<string | null>(null);
  const [target, setTarget] = useState<Reveal | null>(null);
  const [error, setError] = useState('');
  const box = useRef<HTMLDivElement>(null);
  const editor = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);
  const shown = useRef<Reveal | null>(null);
  const save = useRef(onSave);
  const unsaved = useSyncExternalStore(subscribe, unsavedFiles);
  const folder = `nodes/${name}/`;

  useEffect(() => {
    save.current = onSave;
  });

  // shortcut: the cursor and the scroll position are lost when the tab closes; keep the view state if users ask.
  useEffect(() => {
    const e = monaco.editor.create(box.current!, {
      model: null,
      theme: 'patina',
      automaticLayout: true,
      minimap: { enabled: false },
      glyphMargin: true,
      lineNumbersMinChars: 3,
      fontSize: 12,
      fontFamily: "Consolas, 'Cascadia Mono', monospace",
      scrollBeyondLastLine: false,
      fixedOverflowWidgets: true,
    });
    e.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, () => save.current());
    editor.current = e;
    return () => {
      e.dispose();
      editor.current = null;
    };
  }, []);

  useEffect(() => {
    editor.current?.updateOptions({ readOnly });
  }, [readOnly]);

  useEffect(() => {
    let live = true;
    api
      .nodeFiles(name)
      .then((list) => {
        if (!live) return;
        setFiles({ name, list });
        setPath((p) => (p && list.includes(p) ? p : (list.find((f) => f.endsWith('/src/lib.rs')) ?? list[0] ?? null)));
      })
      .catch((e) => live && setError(String(e)));
    return () => {
      live = false;
    };
  }, [name]);

  const go = (at: Reveal | null) => {
    if (!at) return;
    setTarget(at);
    setPath(at.path);
  };
  useEffect(() => go(reveal), [reveal]);

  useEffect(() => {
    if (!path) return;
    let live = true;
    openFile(path)
      .then((model) => {
        const e = editor.current;
        if (!live || !e) return;
        if (e.getModel() !== model) e.setModel(model);
        setError('');
        if (target && target.path === path && shown.current !== target) {
          shown.current = target;
          e.revealLineInCenter(target.line);
          e.setPosition({ lineNumber: target.line, column: target.col });
          e.focus();
        }
      })
      .catch((e) => live && setError(String(e)));
    return () => {
      live = false;
    };
  }, [path, target]);

  const [state, bad] = buildState(run);
  const mine = nodeErrors(run, name);
  const list = files?.name === name ? files.list : [];
  return (
    <div className="code-tab">
      <div className="tabs files" role="tablist" aria-label="Files">
        {list.map((f) => (
          <button key={f} role="tab" aria-selected={f === path} className={f === path ? 'tab active' : 'tab'} title={f} onClick={() => setPath(f)}>
            {f.slice(folder.length)}
            {unsaved.includes(f) && <span className="dot" aria-label="Unsaved changes" />}
          </button>
        ))}
      </div>
      <div className={bad ? 'code-state error-text' : 'code-state label'}>{state}</div>
      <div className="code-box" ref={box} />
      {error && <div className="code-state error-text">{error}</div>}
      {mine.length > 0 && (
        <div className="code-errors" role="alert">
          <div className="code-errors-title">Compile error</div>
          {mine.map((e, i) => (
            <button key={i} className="code-error" onClick={() => go(revealOf(e, Date.now()))}>
              {e.file!.slice(folder.length)} line {e.start[0]}: {errorText(e)}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

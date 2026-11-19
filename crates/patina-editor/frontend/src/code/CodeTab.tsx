// The Code tab of the Inspector (spec 15.4, 15.10): the files of a node or widget folder in
// Monaco, the build state of the saved code and the compile errors of the folder (15.5).

import { useEffect, useRef, useState, useSyncExternalStore } from 'react';
import { api, errorHead, seconds } from '../api';
import { monaco, openFile, subscribe, unsavedFiles } from './code';
import { revealOf, type Reveal } from './reveal';
import { NameField } from '../controls/NameField';
import { StateIcon, type RunState } from '../canvas/NodeView';

/** The build state of the saved code, and true when it is an error. */
export function buildState(run: RunState): [string, boolean] {
  if (run.phase === 'building') return ['Building', false];
  if (run.phase === 'failed') return ['Error', true];
  const built = run.phase === 'ran' || run.phase === 'loaded' || run.phase === 'lost';
  if (built && run.errors.length > 0) return ['Running the last build that compiled', true];
  if (built && run.lastBuildMs !== null) return [`Built in ${seconds(run.lastBuildMs)}`, false];
  return ['Not built', false];
}

// The view of each file and the last file of each folder stay when the tab closes (15.10).
const views = new WeakMap<monaco.editor.ITextModel, monaco.editor.ICodeEditorViewState>();
const lastFile = new Map<string, string>();
/** The last reveal that showed, so a tab that opens again does not show it again. */
let shown: Reveal | null = null;

function keepView(e: monaco.editor.IStandaloneCodeEditor) {
  const model = e.getModel();
  const view = e.saveViewState();
  if (model && view) views.set(model, view);
}

interface Props {
  /** The node or widget folder, for example `nodes/number`. */
  name: string;
  run: RunState;
  readOnly: boolean;
  /** The code revision of the view. A file that came or went from outside the editor shows in the list (15.10). */
  revision: number;
  reveal: Reveal | null;
  onSave: () => void;
  /** Adds a crate to the folder. It fails with the problem of the crate. */
  onAddCrate: (spec: string) => Promise<void>;
}

export function CodeTab({ name, run, readOnly, revision, reveal, onSave, onAddCrate }: Props) {
  const [adding, setAdding] = useState(false);
  const [files, setFiles] = useState<{ name: string; list: string[] } | null>(null);
  const [path, setPath] = useState<string | null>(null);
  const [target, setTarget] = useState<Reveal | null>(null);
  const [error, setError] = useState('');
  const box = useRef<HTMLDivElement>(null);
  const editor = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);
  const save = useRef(onSave);
  const unsaved = useSyncExternalStore(subscribe, unsavedFiles);
  const folder = `${name}/`;

  useEffect(() => {
    save.current = onSave;
  });

  useEffect(() => {
    const el = box.current!;
    const e = monaco.editor.create(el, {
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
    // With errors, the box has the height of the code, so the error well is directly under it (mockup 3).
    e.onDidContentSizeChange(() => (el.style.height = `${e.getContentHeight()}px`));
    editor.current = e;
    return () => {
      keepView(e);
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
      .folderFiles(name)
      .then((list) => {
        if (!live) return;
        const last = lastFile.get(name);
        setFiles({ name, list });
        setPath((p) => (p && list.includes(p) ? p : last && list.includes(last) ? last : (list.find((f) => f.endsWith('/src/lib.rs')) ?? list[0] ?? null)));
      })
      .catch((e) => live && setError(String(e)));
    return () => {
      live = false;
    };
  }, [name, revision]);

  const go = (at: Reveal | null) => {
    if (!at || at === shown) return;
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
        if (e.getModel() !== model) {
          keepView(e);
          e.setModel(model);
          const view = views.get(model);
          if (view) e.restoreViewState(view);
        }
        if (path.startsWith(`${name}/`)) lastFile.set(name, path);
        setError('');
        if (target && target.path === path && shown !== target) {
          shown = target;
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
  const mine = run.errors.filter((e) => e.file?.startsWith(folder));
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
      <div className="code-bar">
        <div className={bad ? 'code-state error-text' : 'code-state label'}>{state}</div>
        {!readOnly && (
          <>
            <button className="btn" title="Save (Ctrl+S)" disabled={!unsaved.some((f) => f.startsWith(folder))} onClick={() => save.current()}>
              Save
            </button>
            <button className="btn" onClick={() => setAdding(true)}>
              Add crate
            </button>
          </>
        )}
      </div>
      {adding && (
        <NameField
          label="Crate, widget or type name"
          onMake={async (spec) => {
            await onAddCrate(spec);
            setPath(`${folder}Cargo.toml`);
          }}
          onClose={() => setAdding(false)}
        />
      )}
      <div className={mine.length > 0 ? 'code-box fit' : 'code-box'} ref={box} />
      {error && <div className="code-state error-text">{error}</div>}
      {mine.length > 0 && (
        <div className="code-errors" role="alert">
          <div className="code-errors-title">
            <span aria-hidden="true">
              <StateIcon state="compile-error" />
            </span>
            {mine.length === 1 ? 'Compile error' : `${mine.length} compile errors`}
          </div>
          {mine.map((e, i) => (
            <button key={i} className="code-error" title={e.file ?? undefined} onClick={() => go(revealOf(e, Date.now()))}>
              {new Set(mine.map((m) => m.file)).size > 1 && `${e.file!.slice(folder.length)} `}
              {errorHead(e)}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

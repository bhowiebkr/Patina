// The code buffers for the parts of the editor outside the Code tab. Monaco and `code.ts` load
// with the first open of the Code tab or a restore, so the start loads less script.

import type { Code, Diagnostic, View } from '../api';

/** The functions of `code.ts` that the rest of the editor calls. */
export interface CodeModule {
  onCodeError(f: (e: string) => void): void;
  showErrors(list: Diagnostic[]): void;
  subscribe(listener: () => void): () => void;
  unsavedFiles(): string[];
  saveCode(): Promise<View | null>;
  reloadCode(): Promise<void>;
  flushCode(): Promise<void>;
  putCode(path: string, text: string): Promise<void>;
  restoreCode(code: Code): Promise<string>;
  closeCode(): void;
  analyzerProject(next: string | null, serial: number): void;
}

const NONE: string[] = [];
let mod: CodeModule | null = null;
let errors: Diagnostic[] = [];
let reportError: (e: string) => void = () => {};
let analyzer: [string | null, number] = [null, 0];
const listeners = new Set<() => void>();
const notify = () => listeners.forEach((f) => f());

/** `code.ts` calls this when it loads. */
export function attachCode(m: CodeModule) {
  mod = m;
  m.onCodeError(reportError);
  m.showErrors(errors);
  m.analyzerProject(...analyzer);
  m.subscribe(notify);
  notify();
}

/** Loads Monaco and the code buffers. */
export function loadCode(): Promise<unknown> {
  return import('./code');
}

/** Sets the function that shows an error of the code buffers, for example of the recovery file. */
export function onCodeError(f: (e: string) => void) {
  reportError = f;
  mod?.onCodeError(f);
}

/** Shows the compile errors of the last build in each open file (15.5). */
export function showErrors(list: Diagnostic[]) {
  errors = list;
  mod?.showErrors(list);
}

export function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** The paths of the files with unsaved changes. The list stays the same object until it changes. */
export function unsavedFiles(): string[] {
  return mod?.unsavedFiles() ?? NONE;
}

/** Sends one SetCode edit for each changed file. Returns the last view, or null with no change. */
export function saveCode(): Promise<View | null> {
  return mod ? mod.saveCode() : Promise.resolve(null);
}

/** Reads the open files again after an edit or an undo of a folder changed them. */
export function reloadCode(): Promise<void> {
  return mod ? mod.reloadCode() : Promise.resolve();
}

let steps: Promise<unknown> = Promise.resolve();

/**
 * Runs an undo or a redo after the Rust side has the newest unsaved text. An undo of a code
 * change writes no file: its text goes into the buffer as an unsaved change (15.10).
 */
export function undoRedo(job: () => Promise<View>): Promise<View> {
  const run = steps.then(async () => {
    await mod?.flushCode();
    const v = await job();
    if (v.codeText) {
      await loadCode();
      await mod!.putCode(v.codeText.path, v.codeText.text);
    }
    return v;
  });
  steps = run.catch(() => undefined);
  return run;
}

/** Puts the text of a recovery file into the buffers (15.10). Returns an error, or ''. */
export async function restoreCode(code: Code): Promise<string> {
  if (Object.keys(code).length === 0) return '';
  await loadCode();
  return mod!.restoreCode(code);
}

/** Closes all buffers when another project opens. */
export function closeCode() {
  errors = [];
  mod?.closeCode();
}

/** Sets the project of rust-analyzer, or null (see `lsp.ts`). */
export function analyzerProject(next: string | null, serial: number) {
  analyzer = [next, serial];
  mod?.analyzerProject(next, serial);
}

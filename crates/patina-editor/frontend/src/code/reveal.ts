// A request to show a line in the Code tab. This file has no Monaco, so the parts of the editor
// outside the Code tab can use it before Monaco loads.

import type { Diagnostic } from '../api';

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

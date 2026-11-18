// The Logs lines of the graph problems and the lost parameters (15.8). A failed build opens the
// Logs panel, so the compile errors show in full.

import { useEffect, useRef } from 'react';
import { folderName, type View } from '../api';
import { infoOf, type RunState } from '../canvas/NodeView';
import { lostParams } from '../canvas/lostPins';
import type { Line } from './Lines';

export function useLogProblems(view: View | null, run: RunState, log: (lines: Line[]) => void, openLogs: () => void) {
  // The texts of the last view, so a problem shows one time until it goes away.
  const last = useRef<{ serial: number; texts: Set<string> }>({ serial: -1, texts: new Set() });
  useEffect(() => {
    if (!view) return;
    const texts = new Set<string>();
    const lines: Line[] = [];
    const add = (text: string, kind: Line['kind']) => {
      texts.add(text);
      const old = last.current.serial === view.serial && last.current.texts.has(text);
      if (!old) lines.push({ text, kind });
    };
    for (const p of view.problems) add(p, 'err');
    const where = view.graphName === 'main' ? '' : ` in the subgraph ${view.graphName}`;
    for (const entry of view.graph.node ?? []) {
      for (const key of lostParams(infoOf(entry, view.nodes), entry.params) ?? []) {
        add(`${folderName(entry)} (${entry.id})${where}: lost parameter ${key} = ${entry.params?.[key]}. The node has no input ${key}, so the run ignores it.`, 'warn');
      }
    }
    last.current = { serial: view.serial, texts };
    if (lines.length === 0) return;
    log(lines);
    if (lines.some((l) => l.kind === 'err')) openLogs();
  }, [view, log, openLogs]);

  const errors = run.errors;
  const failed = run.phase === 'failed' || run.phase === 'lost';
  useEffect(() => {
    if (failed || errors.length > 0) openLogs();
  }, [failed, errors, openLogs]);
}

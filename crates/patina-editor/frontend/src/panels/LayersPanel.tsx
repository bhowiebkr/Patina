// The Layers panel (spec 7.3, 15.2): the compilation base and slot. It is not an effects stack.

import { seconds } from '../api';
import { layerRows } from './layers';
import type { RunState } from '../canvas/NodeView';

export function LayersPanel({ run }: { run: RunState }) {
  return (
    <ul className="layers" aria-label="Layers">
      {layerRows(run).map((row) => (
        <li key={row.name} className="layer">
          <b>{row.name}</b>
          <span className={row.failed ? 'layer-state failed' : 'layer-state'}>{row.state}</span>
          {row.ms !== null && <span className="hint">Built in {seconds(row.ms)}</span>}
        </li>
      ))}
    </ul>
  );
}

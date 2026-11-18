// The Subgraph interface (mockup 11, spec 15): the public pins of the open subgraph in pin order
// (6.4), and a preview of the node that each use shows.

import type { FocusEvent, KeyboardEvent } from 'react';
import { newId, SHAPES, type Edit, type Point, type PublicInput, type PublicOutput, type Shape, type View } from '../api';
import { SymbolBody, symbolLayout } from '../canvas/SymbolView';

type Pin = PublicInput | PublicOutput;

const Arrow = ({ d }: { d: string }) => (
  <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.5" aria-hidden="true">
    <path d={d} />
  </svg>
);

const SHAPE_NAMES: Record<Shape, string> = {
  and: 'AND gate',
  nand: 'NAND gate',
  or: 'OR gate',
  nor: 'NOR gate',
  xor: 'XOR gate',
  xnor: 'XNOR gate',
  not: 'NOT gate',
  buffer: 'Buffer',
  mux: 'Multiplexer',
  alu: 'ALU',
  adder: 'Adder',
  chip: 'Chip',
  bus: 'Bus bar',
};

/** The **Shape** and the **Label** of each use of the open subgraph, in the **Look** section (4.3). */
export function SymbolFields({ view, onEdit }: { view: View; onEdit: (edit: Edit) => void }) {
  const symbol = view.graph.symbol;
  const commit = (e: FocusEvent<HTMLInputElement>) => {
    const label = e.currentTarget.value.trim();
    if (symbol && label !== (symbol.label ?? '')) onEdit({ kind: 'SetSymbol', symbol: { shape: symbol.shape, label: label || null } });
  };
  const keys = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Escape') e.currentTarget.value = symbol?.label ?? '';
    if (e.key === 'Enter' || e.key === 'Escape') e.currentTarget.blur();
  };
  return (
    <>
      <label className="param">
        <span className="label">Shape</span>
        <select
          className="field"
          value={symbol?.shape ?? 'box'}
          disabled={!view.trusted}
          onChange={(e) => {
            const shape = e.currentTarget.value;
            onEdit({ kind: 'SetSymbol', symbol: shape === 'box' ? null : { shape: shape as Shape, label: symbol?.label ?? null } });
          }}
        >
          <option value="box">Box</option>
          {SHAPES.map((s) => (
            <option key={s} value={s}>
              {SHAPE_NAMES[s]}
            </option>
          ))}
        </select>
      </label>
      {symbol && (
        <label className="param">
          <span className="label">Label</span>
          <input key={symbol.label ?? ''} className="field" defaultValue={symbol.label ?? ''} placeholder={view.graphName.toUpperCase()} maxLength={32} disabled={!view.trusted} onBlur={commit} onKeyDown={keys} />
        </label>
      )}
    </>
  );
}

export function SubgraphInterface({ view, onEdit }: { view: View; onEdit: (edit: Edit) => void }) {
  const info = view.nodes.find((n) => n.subgraph && n.name === view.graphName);
  const inputs = view.graph.input ?? [];
  const outputs = view.graph.output ?? [];
  const xs = (view.graph.node ?? []).map((n) => n.pos?.[0] ?? 0);
  const add = (input: boolean) => {
    const last = (input ? inputs : outputs).at(-1)?.pos;
    const pos: Point = last ? [last[0], last[1] + 60] : [input ? Math.min(0, ...xs) - 250 : Math.max(0, ...xs) + 350, 0];
    onEdit({ kind: 'AddPin', key: newId(4), name: input ? 'input' : 'output', input, pos });
  };
  const list = (input: boolean, pins: readonly Pin[]) => {
    const what = input ? 'input' : 'output';
    const types = input ? info?.inputs : info?.outputs;
    return (
      <div className="section">
        <h3>
          {input ? 'Inputs' : 'Outputs'} ({pins.length})
        </h3>
        {pins.map((p, i) => {
          const commit = (e: FocusEvent<HTMLInputElement>) => {
            const name = e.currentTarget.value.trim();
            if (name === '') e.currentTarget.value = p.name;
            else if (name !== p.name) onEdit({ kind: 'RenamePin', key: p.key, name });
          };
          const keys = (e: KeyboardEvent<HTMLInputElement>) => {
            if (e.key === 'Escape') e.currentTarget.value = p.name;
            if (e.key === 'Enter' || e.key === 'Escape') e.currentTarget.blur();
          };
          return (
            <div key={p.key} className="pin-row" data-pin={p.key}>
              <input key={p.name} className="field" defaultValue={p.name} aria-label={`Name of ${what} ${i + 1}`} disabled={!view.trusted} onBlur={commit} onKeyDown={keys} />
              <span className="label pin-type">{types?.find((t) => t.key === p.key)?.ty ?? '?'}</span>
              <button className="icon-btn" aria-label={`Move ${p.name} up`} title="Move up" disabled={!view.trusted || i === 0} onClick={() => onEdit({ kind: 'OrderPin', key: p.key, index: i - 1 })}>
                <Arrow d="M6 10V2M2 6l4-4 4 4" />
              </button>
              <button className="icon-btn" aria-label={`Move ${p.name} down`} title="Move down" disabled={!view.trusted || i === pins.length - 1} onClick={() => onEdit({ kind: 'OrderPin', key: p.key, index: i + 1 })}>
                <Arrow d="M6 2v8M2 6l4 4 4-4" />
              </button>
              <button className="icon-btn" aria-label={`Remove ${p.name}`} title="Remove" disabled={!view.trusted} onClick={() => onEdit({ kind: 'RemovePin', key: p.key })}>
                <Arrow d="M2 2l8 8M10 2l-8 8" />
              </button>
            </div>
          );
        })}
        <button className="btn" disabled={!view.trusted} onClick={() => add(input)}>
          Add {what}
        </button>
      </div>
    );
  };
  return (
    <>
      {list(true, inputs)}
      {list(false, outputs)}
      <div className="section">
        <h3>Public node preview</h3>
        {view.graph.symbol && info ? (
          <div className="symbol-preview" aria-label="Public node preview">
            <SymbolBody layout={symbolLayout(view.graph.symbol, view.graphName, info.inputs, info.outputs)} far={false} />
          </div>
        ) : (
          <div className="pin-preview" aria-label="Public node preview">
            <div className="pin-preview-title">{view.graphName}</div>
            <div className="pin-preview-body">
              <div>{inputs.map((p) => <div key={p.key}>{p.name}</div>)}</div>
              <div>{outputs.map((p) => <div key={p.key}>{p.name}</div>)}</div>
            </div>
          </div>
        )}
      </div>
    </>
  );
}

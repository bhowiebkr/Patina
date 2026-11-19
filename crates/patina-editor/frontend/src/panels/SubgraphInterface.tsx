// The Subgraph interface (mockup 11, spec 15): the public pins of the open subgraph in pin order
// (6.4), and a preview of the node that each use shows.

import type { FocusEvent, KeyboardEvent, ReactNode } from 'react';
import { newId, SHAPES, type Edit, type NodeInfo, type Point, type PublicInput, type PublicOutput, type Shape, type Symbol, type View } from '../api';
import { kindOf } from '../canvas/bus';
import { outputType } from '../canvas/NodeView';
import { SymbolBody } from '../canvas/SymbolView';
import { isOutline, startOutline, symbolLayout } from '../canvas/symbols';
import { ColorInput } from '../controls/ValueWidgets';
import { LookEditor } from './LookEditor';
import { makeFace } from './faceCode';

type Pin = PublicInput | PublicOutput;

/** The lane count of the kind of a pin of the preview, by "i:<key>" or "o:<key>" (15.9). A generic output has the kind of its wire source. */
function lanesOf(view: View, key: string, info: NodeInfo): number {
  const [output, k] = [key.startsWith('o:'), key.slice(2)];
  const ty = (output ? info.outputs : info.inputs).find((p) => p.key === k)?.ty;
  const from = output ? view.graph.output?.find((p) => p.key === k)?.from : undefined;
  return (kindOf(view.kinds, ty) ?? (from ? kindOf(view.kinds, outputType(view, from)) : undefined))?.lanes.length ?? 0;
}

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
  custom: 'Custom outline',
  code: 'Code',
};

/** The subgraph node of the open subgraph, with its pin types. */
const infoOf = (view: View) => view.nodes.find((n) => n.subgraph && n.name === view.graphName);

/** The **Shape**, the **Color** and the **Label** of each use of the open subgraph, in the **Look** section (4.3). */
export function SymbolFields({ view, onEdit, onJob }: { view: View; onEdit: (edit: Edit) => void; onJob: (job: Promise<View>) => void }) {
  const symbol = view.graph.symbol;
  const set = (next: Symbol) => onEdit({ kind: 'SetSymbol', symbol: next });
  const commit = (e: FocusEvent<HTMLInputElement>) => {
    const label = e.currentTarget.value.trim();
    if (symbol && label !== (symbol.label ?? '')) set({ ...symbol, label: label || null });
  };
  // Only the custom shape keeps an outline. A new custom shape starts as a rectangle with room for the pins.
  const shapeOf = (shape: Shape): Symbol => {
    const next: Symbol = { shape, label: symbol?.label ?? null, color: symbol?.color ?? null };
    const info = infoOf(view);
    if (shape !== 'custom') return next;
    if (isOutline(symbol?.outline)) return { ...next, outline: symbol.outline };
    return { ...next, outline: startOutline(next, view.graphName, info?.inputs ?? [], info?.outputs ?? [], (key) => (info ? lanesOf(view, key, info) : 0)) };
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
            const info = infoOf(view);
            if (shape === 'code') return onJob(makeFace(view, info, (key) => (info ? lanesOf(view, key, info) : 0)));
            const next = shape === 'box' ? null : shapeOf(shape as Shape);
            // The face node goes with the code shape. Its folder and its code stay.
            onEdit(symbol?.shape === 'code' && symbol.face ? { kind: 'Group', edits: [{ kind: 'SetSymbol', symbol: next }, { kind: 'RemoveNode', id: symbol.face }] } : { kind: 'SetSymbol', symbol: next });
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
      {symbol && symbol.shape !== 'bus' && symbol.shape !== 'code' && (
        <div className="param look-color">
          <span className="label">Color</span>
          <ColorInput value={symbol.color ?? (symbol.shape === 'chip' ? '#1c1c1b' : '#262626')} label="Body color" send={(color) => set({ ...symbol, color })} disabled={!view.trusted} />
          {symbol.color && (
            <button className="btn" disabled={!view.trusted} onClick={() => set({ ...symbol, color: null })}>
              Theme color
            </button>
          )}
        </div>
      )}
    </>
  );
}

/** `code` gives the code editor of a folder, for the face node of the code shape (4.3). */
export function SubgraphInterface({ view, onEdit, code }: { view: View; onEdit: (edit: Edit) => void; code: (folder: string) => ReactNode }) {
  const info = infoOf(view);
  const symbol = view.graph.symbol;
  const face = symbol?.shape === 'code' ? view.graph.node?.find((n) => n.id === symbol.face)?.folder : undefined;
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
        <h3>{face ? 'Look code' : 'Public node preview'}</h3>
        {face ? (
          <>
            <p className="label look-hint">The node {face.slice(face.indexOf('/') + 1)} in this subgraph draws each use. Its ui draws the body, and each ui.pin puts a public pin. Save to build it.</p>
            <div className="look-code">{code(face)}</div>
          </>
        ) : symbol && info && symbol.shape === 'custom' && isOutline(symbol.outline) ? (
          <>
            <div className="symbol-preview" aria-label="Public node preview">
              <LookEditor
                outline={symbol.outline}
                layoutOf={(outline) => symbolLayout({ ...symbol, outline }, view.graphName, info.inputs, info.outputs, (key) => lanesOf(view, key, info))}
                disabled={!view.trusted}
                onChange={(outline) => onEdit({ kind: 'SetSymbol', symbol: { ...symbol, outline } })}
              />
            </div>
            <p className="label look-hint">Drag a corner to move it. Drag a dot on an edge to add a corner. Double-click a corner, or select it and press Delete, to remove it.</p>
            <button
              className="btn"
              disabled={!view.trusted}
              onClick={() => onEdit({ kind: 'SetSymbol', symbol: { ...symbol, outline: startOutline(symbol, view.graphName, info.inputs, info.outputs, (key) => lanesOf(view, key, info)) } })}
            >
              Reset outline
            </button>
          </>
        ) : symbol && info ? (
          <div className="symbol-preview" aria-label="Public node preview">
            <SymbolBody layout={symbolLayout(symbol, view.graphName, info.inputs, info.outputs, (key) => lanesOf(view, key, info))} far={false} />
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

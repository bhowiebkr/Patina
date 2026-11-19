// The symbol step of the self-check (check.ts): a node with a circuit symbol, and the problem of an
// unwired input whose type has no parameter support (4.3, 4.8).

import { api, type View } from '../api';
import { changeLayout } from '../panels/Dock';
import { openPanel } from '../panels/layout';
import { BUILD_MS, RUN_MS, nodeToml, type CheckContext } from './check-fixtures';

/** An AND gate with an input that has no parameter support, so it needs a wire. */
const GATE_NODE = `use patina_sdk::node;

/// An AND gate that the self-check adds.
#[node(symbol = "and")]
pub fn gate(a: bool, bytes: &Vec<u8>) -> bool {
    a && !bytes.is_empty()
}
`;

const G = 'f4ee000000000002';

/** Adds the gate, checks its symbol and its problem, and removes it. */
export async function checkSymbol({ step, until, steps }: CheckContext, v: View) {
  await api.checkWrite('nodes/gate/Cargo.toml', nodeToml('gate'));
  await api.checkWrite('nodes/gate/src/lib.rs', GATE_NODE);
  const gateItem = () => [...document.querySelectorAll('[aria-label=Project] .item')].find((b) => b.textContent?.startsWith('gate'));
  await until('the gate node folder', BUILD_MS, () => !!gateItem());
  v = await step('add a gate with a symbol', () => api.edit({ kind: 'AddNode', id: G, folder: 'nodes/gate', pos: [0, 480] }));
  const node = `.react-flow__node[data-id="${G}"] .patina-node`;
  await until('the AND symbol of the gate', BUILD_MS, () => !!document.querySelector(`${node}.symbol svg.symbol`));
  steps.push('a node with a symbol shows the symbol');
  // The input `bytes` has no wire and no parameter, so the node shows a problem, not a compile error of the code.
  const message = () => document.querySelector(`${node} .patina-node-message`)?.textContent ?? '';
  await until('the problem of the unwired input', BUILD_MS, () => message().includes('connect a wire to the input `bytes`'));
  if (!v.graph.node?.some((n) => n.id === G)) throw new Error('the gate is not in the graph');
  steps.push('an unwired input with no parameter support shows a node problem');
  // The problem link of the status bar opens Logs (15.5). The first problem opened Logs, so show Output first.
  const logs = () => document.querySelector('[role=log][aria-label="Logs"]');
  const shown = () => (logs()?.getBoundingClientRect().width ?? 0) > 0;
  changeLayout((l) => openPanel(l, 'output'));
  await until('the Output tab over Logs', RUN_MS, () => !shown());
  const link = () => [...document.querySelectorAll<HTMLButtonElement>('.status-link')].find((b) => b.textContent === 'Errors: 1');
  await until('the problem link of the status bar', RUN_MS, () => !!link());
  link()!.click();
  await until('the problem in Logs from the status link', RUN_MS, () => shown() && logs()!.textContent!.includes('connect a wire to the input `bytes`'));
  steps.push('the problem link of the status bar opens Logs');
  v = await step('remove the gate', () => api.undo());
  if (v.graph.node?.some((n) => n.id === G)) throw new Error('the undo did not remove the gate');
  await api.checkWrite('nodes/gate', null);
  await until('no gate node folder', BUILD_MS, () => !gateItem());
  return v;
}

// The thread step of the self-check (check.ts): two heavy nodes that do not wait for each other run
// on two threads of the worker pool, and the Debug tab shows a row for each thread (4.1, 9.2, 15.7).

import { api, type View } from '../api';
import { BUILD_MS, RUN_MS, nodeToml, tab, type CheckContext } from './check-fixtures';

/** A node that sleeps. A light node shares a task with the other nap, so it is heavy. */
const NAP_NODE = (name: string) => `/// Sleeps for the thread check.
#[patina_sdk::node(heavy)]
pub fn ${name}() {
    std::thread::sleep(std::time::Duration::from_millis(100));
    println!("${name} done");
}
`;

const NAPS = [
  ['nap1', 'f4ee000000000007'],
  ['nap2', 'f4ee000000000008'],
] as const;

/** Adds two naps, runs them, reads their thread rows in the Debug tab and removes them. */
export async function checkThreads({ step, until, result, mark, fresh, steps }: CheckContext, v: View) {
  for (const [name] of NAPS) {
    await api.checkWrite(`nodes/${name}/Cargo.toml`, nodeToml(name));
    await api.checkWrite(`nodes/${name}/src/lib.rs`, NAP_NODE(name));
  }
  const item = (name: string) => [...document.querySelectorAll('[aria-label=Project] .item')].find((b) => b.textContent?.startsWith(name));
  await until('the nap node folders', BUILD_MS, () => NAPS.every(([name]) => !!item(name)));
  v = await step('add two naps', () =>
    api.edit({ kind: 'Group', edits: NAPS.map(([name, id], i) => ({ kind: 'AddNode' as const, id, folder: `nodes/${name}`, pos: [i * 320, 960] as [number, number] })) }),
  );
  // The thread row of each nap call. The two calls of one run are on two rows.
  const rowOf = (name: string) =>
    [...document.querySelectorAll('.debug-row')].find((r) => [...r.querySelectorAll('.debug-call')].some((c) => c.textContent === name))?.querySelector('.debug-thread')?.textContent;
  const rows = () => NAPS.map(([name]) => rowOf(name));
  // The pool runs a domain only when it was faster in the runs before (9.2). A slow run makes the pool faster for the next run.
  for (let run = 1; ; run++) {
    const from = mark();
    v = await step(`run the naps (${run})`, () => api.run());
    await result(v, from, 'nap2 done', BUILD_MS);
    // The naps can end in any order.
    await until('the line of nap1', RUN_MS, () => fresh().includes('nap1 done'));
    tab('Debug')!.click();
    await until('the two nap rows in the Debug tab', RUN_MS, () => rows().every((r) => r !== undefined));
    const [a, b] = rows();
    tab('Output')!.click();
    if (a !== b) {
      steps.push(`two naps run on ${a} and ${b} in run ${run}`);
      break;
    }
    if (run === 3) throw new Error(`the two naps ran on ${a} in each of 3 runs`);
  }

  v = await step('remove the naps', () => api.undo());
  if (v.graph.node?.some((n) => NAPS.some(([, id]) => n.id === id))) throw new Error('the undo did not remove the naps');
  for (const [name] of NAPS) await api.checkWrite(`nodes/${name}`, null);
  await until('no nap node folders', BUILD_MS, () => NAPS.every(([name]) => !item(name)));
  return v;
}

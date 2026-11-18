// The Package step of the self-check (check.ts): the Package button of the Project panel makes a
// package of the saved project in the folder of the folder dialog, and the Open form opens the
// package as its original project (14.1).

import { api } from '../api';
import { BUILD_MS, RUN_MS, button, enterText, leftTab, topButton, type CheckContext } from './check-fixtures';

/** Saves the project, clicks Package and opens the package. The check gives a folder of `dir` in place of the folder dialog. */
export async function checkPackage({ step, until, steps }: CheckContext, dir: string) {
  const saved = await step('save before the package', () => api.save());
  const parent = `${dir}/package`;
  const text = () => document.body.textContent ?? '';
  // The editor of the self-check can run without patina-cli next to it. Then Package shows that error.
  const missing = 'Patina cannot make a package, because';
  const pickFolder = api.pickFolder;
  api.pickFolder = async () => parent;
  try {
    leftTab('Project').click();
    button('Package')!.click();
    await until('the package of the project', BUILD_MS, () => text().includes(`Made the package ${parent}`) || text().includes(missing));
  } finally {
    api.pickFolder = pickFolder;
  }
  if (text().includes(missing)) {
    steps.push('Package shows that patina-cli is not next to the editor');
    return;
  }
  steps.push('Package makes a package of the saved project');
  // The package of a trusted project is a new folder, so it opens with the trust banner and builds nothing (11).
  topButton('Close')!.click();
  const openForm = () => [...document.querySelectorAll('form.start-box')].find((f) => f.querySelector('h2')?.textContent === 'Open project');
  const openButton = () => [...(openForm()?.querySelectorAll('button') ?? [])].find((b) => b.textContent === 'Open');
  await until('the Open project form after Close', RUN_MS, () => !!openButton());
  enterText(openForm()!.querySelector('input')!, `${parent}/${saved.root.split(/[\\/]/).pop()}`);
  await until('the Open button on', RUN_MS, () => openButton()?.disabled === false);
  openButton()!.click();
  await until('the package project with the trust banner', BUILD_MS, () => !!button('Trust project') && topButton('Run')?.disabled === true);
  // Open graph of the same graph changes nothing and gives the view of the open project.
  const v = await api.openGraph(saved.graphName);
  if (!/[\\/]project$/.test(v.root) || JSON.stringify(v.graph) !== JSON.stringify(saved.graph)) {
    throw new Error(`Open project on the package did not open its original project: ${v.root}`);
  }
  steps.push('Open project on the package opens its original project');
}

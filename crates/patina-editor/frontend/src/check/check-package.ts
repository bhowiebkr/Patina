// The Package step of the self-check (check.ts): the Package button of the Project panel makes a
// package of the saved project in the folder of the folder dialog (14.1).

import { api } from '../api';
import { BUILD_MS, button, leftTab, type CheckContext } from './check-fixtures';

/** Saves the project and clicks Package. The check gives a folder of `dir` in place of the folder dialog. */
export async function checkPackage({ step, until, steps }: CheckContext, dir: string) {
  await step('save before the package', () => api.save());
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
  steps.push(text().includes(missing) ? 'Package shows that patina-cli is not next to the editor' : 'Package makes a package of the saved project');
}

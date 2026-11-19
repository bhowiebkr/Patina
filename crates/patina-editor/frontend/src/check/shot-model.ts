// The model workbench: the 3D view in the second center pane (tasks D5 and D8), checked on the hidden screenshot desktop.
import { api, onEngine, type View } from '../api';
import { changeLayout } from '../panels/Dock';
import { closePanel, openView, VIEW_SLOTS } from '../panels/layout';
import { closeMenu, rightClick } from './check-fixtures';

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));
const panel = () => document.querySelector<HTMLElement>('aside[data-view="model-view"]');
// The text of the shown solid, for example "144 triangles, bounds -1.2 -1.2 0 to 1.2 1.2 2.933".
const shown = () => panel()?.innerText.match(/\d+ triangles, bounds [^\n]+/)?.[0]?.trim() ?? '';
const SEED = '5eed000000000001';
const UNION = 'a11a000000000008';

export async function modelWorkbench(view: View, show: (view: View) => void): Promise<string[]> {
  if (!view.views.includes('model-view')) return [];
  let stage = 'open';
  let failure = '';
  let runs = 0;
  const off = await onEngine((event) => {
    if (event.state === 'lost' || event.state === 'failed') failure = event.text;
    if (event.state === 'ran') {
      runs++;
      if (!event.ok) failure = JSON.stringify(event.reports);
    }
  });
  const until = async (test: () => boolean, message: string) => {
    const end = Date.now() + 150_000;
    while (!test()) {
      const error = failure || panel()?.querySelector('.error-text')?.textContent;
      if (error) throw new Error(error);
      if (Date.now() > end) throw new Error(message);
      await sleep(100);
    }
  };
  const closeViews = () => changeLayout((layout) => VIEW_SLOTS.reduce((next, slot) => closePanel(next, slot), layout));
  try {
    const steps: string[] = [];
    closeViews();
    show(await api.stop());
    show(await api.openGraph('main'));
    const beforeRun = runs;
    show(await api.run());
    await until(() => runs > beforeRun, 'The tower graph did not run');
    stage = 'open the view';
    changeLayout((layout) => openView(VIEW_SLOTS.reduce((next, slot) => closePanel(next, slot), layout), 'model-view', 'split'));
    await until(() => !!panel()?.querySelector('.gui-viewport'), 'The 3D view did not build');
    if (!panel()!.closest('section.area.split')) throw new Error('The 3D view did not open in the second center pane');
    // The first read of the union output starts a batch run, so no second Run is necessary (15.8 rule 3).
    stage = 'select a node';
    rightClick(document.querySelector(`.react-flow__node[data-id="${UNION}"]`)!);
    await until(() => !!document.querySelector('.menu-layer'), 'The node menu did not open');
    closeMenu();
    await until(() => !document.querySelector('.menu-layer') && !!panel()?.textContent?.includes('union - ') && shown() !== '', 'The 3D view did not show the selected union');
    const first = shown();
    await sleep(800);
    // Only a capture with an area shows the 3D surface under the page (15.8).
    const capture = async (name: string) => {
      const r = panel()!.querySelector('.gui-viewport')!.getBoundingClientRect();
      const drawn = await api.capture(name, [r.left, r.top, r.width, r.height].map((v) => v * devicePixelRatio));
      if (drawn !== null && drawn < 0.9) throw new Error(`The 3D view drew ${Math.round(drawn * 100)} % of its viewport`);
    };
    await capture('model-view');
    steps.push(`the 3D view in the second pane shows the selected union: ${first}`);
    stage = 'change the seed';
    show(await api.edit({ kind: 'SetParam', id: SEED, key: 'seed', text: '8' }));
    await until(() => shown() !== '' && shown() !== first, `A new seed did not give a new mesh in the 3D view: ${first}`);
    const second = shown();
    await sleep(800);
    await capture('model-view-seed-8');
    show(await api.undo());
    await until(() => shown() === first, 'Undo of the seed did not give the first mesh again');
    steps.push(`seed 8 gives a new mesh (${second}), and undo gives the first mesh again`);
    closeViews();
    show(await api.stop());
    return steps;
  } catch (error) {
    await sleep(600);
    await api.capture('model-failed');
    throw new Error(`model workbench, ${stage}: ${error}`);
  } finally {
    off();
  }
}

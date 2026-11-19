// The model, the city and the gun workbenches: the 3D view in the second center pane (tasks D5, D8, E6 and F5), checked on the hidden screenshot desktop.
import { api, onEngine, type View } from '../api';
import { changeLayout } from '../panels/Dock';
import { closePanel, openView, VIEW_SLOTS } from '../panels/layout';
import { closeMenu, rightClick } from './check-fixtures';

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));
const panel = () => document.querySelector<HTMLElement>('aside[data-view="model-view"]');
// The text of the shown solid, for example "144 triangles, bounds -1.2 -1.2 0 to 1.2 1.2 2.933".
const shown = () => panel()?.innerText.match(/\d+ triangles, bounds [^\n]+/)?.[0]?.trim() ?? '';
// The seed node, the node that the check selects, the first name of the captures, and a capture with Toon.
// `next` is a workbench beside this one that the check opens while the 3D view shows, when the folder is there.
// `wall` is a view in the bottom dock whose click sets the seed and selects `node` again.
const TARGETS: { seed: string; node: string; title: string; shot: string; toon: boolean; next?: string; wall?: string }[] = [
  { seed: '5eed000000000001', node: 'a11a000000000008', title: 'union', shot: 'model', toon: false },
  { seed: 'c170000000000001', node: 'c170000000000017', title: 'merge', shot: 'city', toon: false, next: 'gun' },
  { seed: '9a70000000000001', node: '9a70000000000002', title: 'look', shot: 'gun', toon: true, wall: 'gun-wall' },
];

export async function modelWorkbench(view: View, show: (view: View) => void): Promise<string[]> {
  const target = TARGETS.find((t) => view.graph.node?.some((n) => n.id === t.node));
  if (!view.views.includes('model-view') || !target) return [];
  const { seed, node, title, shot, toon, next, wall } = target;
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
    await until(() => runs > beforeRun, 'The main graph did not run');
    stage = 'open the view';
    changeLayout((layout) => openView(VIEW_SLOTS.reduce((next, slot) => closePanel(next, slot), layout), 'model-view', 'split'));
    await until(() => !!panel()?.querySelector('.gui-viewport'), 'The 3D view did not build');
    if (!panel()!.closest('section.area.split')) throw new Error('The 3D view did not open in the second center pane');
    // The first read of the selected node starts a batch run, so no second Run is necessary (15.8 rule 3).
    stage = 'select a node';
    rightClick(document.querySelector(`.react-flow__node[data-id="${node}"]`)!);
    await until(() => !!document.querySelector('.menu-layer'), 'The node menu did not open');
    closeMenu();
    await until(() => !document.querySelector('.menu-layer') && !!panel()?.textContent?.includes(`${title} - `) && shown() !== '', `The 3D view did not show the selected ${title}`);
    // Two nodes can have the same name, for example the two `look` nodes of the gun, so let the view take the selection first.
    await sleep(800);
    const first = shown();
    // Only a capture with an area shows the 3D surface under the page (15.8). It gives a print of the image.
    const capture = async (name: string) => {
      const r = panel()!.querySelector('.gui-viewport')!.getBoundingClientRect();
      const [drawn, print] = (await api.captureArea(name, [r.left, r.top, r.width, r.height].map((v) => v * devicePixelRatio))) ?? [1, name];
      if (drawn < 0.9) throw new Error(`The 3D view drew ${Math.round(drawn * 100)} % of its viewport`);
      return print;
    };
    const firstPrint = await capture(`${shot}-view`);
    steps.push(`the 3D view in the second pane shows the selected ${title}: ${first}`);
    if (toon) {
      stage = 'toon';
      const box = () => [...panel()!.querySelectorAll('label.gui-row')].find((l) => l.textContent === 'Toon')?.querySelector('input');
      box()!.click();
      await until(() => !!box()?.checked, 'The Toon toggle did not turn on');
      await sleep(800);
      await capture(`${shot}-view-toon`);
      box()!.click();
      await until(() => box()?.checked === false, 'The Toon toggle did not turn off');
      steps.push('the Toon toggle draws the toon look');
    }
    stage = 'change the seed';
    show(await api.edit({ kind: 'SetParam', id: seed, key: 'seed', text: '8' }));
    await until(() => shown() !== '' && shown() !== first, `A new seed did not give a new mesh in the 3D view: ${first}`);
    const second = shown();
    await sleep(800);
    if ((await capture(`${shot}-view-seed-8`)) === firstPrint) throw new Error('The 3D view drew the same image for seed 8');
    show(await api.undo());
    await until(() => shown() === first, 'Undo of the seed did not give the first mesh again');
    steps.push(`seed 8 gives a new mesh (${second}) and a new image, and undo gives the first mesh again`);
    if (wall) {
      stage = `click the ${wall}`;
      changeLayout((layout) => openView(layout, wall, 'bottom'));
      const pad = () => document.querySelector<SVGSVGElement>(`aside[data-view="${wall}"] svg.gui-canvas`);
      await until(() => !!pad(), `The ${wall} view did not build`);
      // Select the seed node first, so that only the click can select the node of the 3D view again.
      // The 3D view keeps its node, because the seed has no model, so the check reads the canvas.
      const selected = (id: string) => !!document.querySelector(`.react-flow__node[data-id="${id}"].selected`);
      rightClick(document.querySelector(`.react-flow__node[data-id="${seed}"]`)!);
      await until(() => !!document.querySelector('.menu-layer'), 'The node menu did not open');
      closeMenu();
      await until(() => selected(seed) && !selected(node), 'The right click did not select the seed node');
      // The first tile, at the top left, is the first seed of the page. The seed of the graph is the last one.
      const box = pad()!.getBoundingClientRect();
      const at = { bubbles: true, button: 0, pointerId: 1, isPrimary: true, clientX: box.left + 8, clientY: box.top + 8 };
      pad()!.dispatchEvent(new PointerEvent('pointerdown', at));
      pad()!.dispatchEvent(new PointerEvent('pointerup', at));
      await until(() => selected(node) && !selected(seed) && shown() !== '' && shown() !== first, `A click on the ${wall} did not select ${title} and show its tile`);
      steps.push(`a click on the first tile of the ${wall} selects ${title} and shows a new mesh: ${shown()}`);
      show(await api.undo());
      await until(() => shown() === first, `Undo of the ${wall} click did not give the first mesh again`);
    }
    if (next) {
      // A new node process draws the view after the switch. A view that keeps the old image fails the seed check of the nested run.
      stage = `open the ${next} workbench`;
      const opened = await api.openProject(`${view.root}/../${next}`).catch(() => null);
      if (opened) {
        show(opened.trusted ? opened : await api.trustProject());
        const nested = await modelWorkbench(await api.openGraph('main'), show);
        steps.push(`after the switch from ${shot}, the 3D view of the ${next} workbench draws its own images`, ...nested.map((s) => `${next}: ${s}`));
        show(await api.openProject(view.root));
      }
    }
    closeViews();
    show(await api.stop());
    return steps;
  } catch (error) {
    await sleep(600);
    await api.capture(`${shot}-failed`);
    // The 3D view shows its errors as labels, for example an error of the surface.
    throw new Error(`${shot} workbench, ${stage}: ${error}. The 3D view shows: ${panel()?.innerText.replace(/\s+/g, ' ')}`);
  } finally {
    off();
  }
}

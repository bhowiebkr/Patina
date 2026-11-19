// The arrangement workbench, exercised only on the hidden check desktop.
import { api, onEngine, type View } from '../api';
import { changeLayout } from '../panels/Dock';
import { openPanel, resizeArea } from '../panels/layout';
import { enterText } from './check-fixtures';
import { click, field, fit, showTab } from './shot-synth';

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));
const panel = () => document.querySelector<HTMLElement>('aside[data-view="arrange"]');
const scoreText = (view: View) => view.graph.node?.find((n) => n.id === 'e6bf000000000001')?.params?.score;
async function until(test: () => boolean, message: string, failure: () => string = () => '') {
  const end = Date.now() + 150_000;
  while (!test()) {
    const error = failure() || panel()?.querySelector('.error-text')?.textContent;
    if (error) throw new Error(error);
    if (Date.now() > end) throw new Error(message);
    await sleep(100);
  }
}

export async function songWorkbench(view: View, show: (view: View) => void): Promise<string[]> {
  if (!view.views.includes('arrange')) return [];
  let frames = 0;
  let failure = '';
  const off = await onEngine((event) => {
    if (event.state === 'failed' || event.state === 'lost') failure = event.text;
    if (event.state === 'frame') frames++;
  });
  try {
    const steps: string[] = [];
    show(await api.stop()); show(await api.openGraph('song'));
    show(await api.edit({ kind: 'SetParam', id: 'e6bf00000000001e', key: 'audible', text: 'false' }));
    show(await api.run());
    await until(() => frames > 5, 'Song audio did not run', () => failure);
    await showTab('audio');
    changeLayout((l) => openPanel(l, 'project'));
    await showTab('arrange');
    changeLayout((l) => resizeArea(l, 'bottom', 450));
    await until(() => !!panel()?.querySelector('.gui-timeline'), 'No arrangement timeline', () => failure);
    fit('arrange');
    const body = panel()!.querySelector('.view-controls')!.getBoundingClientRect();
    const tracks = panel()!.querySelectorAll('.tl-head');
    if (tracks.length !== 6 || tracks[5].getBoundingClientRect().bottom > body.bottom + 1) throw new Error('The six-track timeline does not fit the 450 px dock');
    if (!panel()!.querySelector('.tl-clip')!.getAttribute('aria-label')?.includes('beats')) throw new Error('Musical clips must announce beat units');
    await api.capture('song-arrangement');
    const before = await api.pause(); show(before);
    const first = panel()!.querySelector<HTMLElement>('.tl-clip')!;
    first.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
    await sleep(350);
    const moved = await api.pause(); show(moved);
    if (!scoreText(moved)?.includes('32.25')) throw new Error('Clip move did not reach the score');
    const undone = await api.undo(); show(undone);
    if (scoreText(undone) !== scoreText(before)) throw new Error('Clip undo did not restore the score');
    steps.push('the musical timeline edits the saved score and Undo restores it');

    const zoom = panel()!.querySelector<HTMLSelectElement>('[aria-label="Timeline zoom"]')!;
    zoom.value = '4'; zoom.dispatchEvent(new Event('change', { bubbles: true }));
    await until(() => panel()!.textContent!.includes('Bars 1 - 8'), 'Timeline zoom did not change the visible bars');
    zoom.value = '1'; zoom.dispatchEvent(new Event('change', { bubbles: true }));
    click('arrange', 'Add track');
    await sleep(350);
    const added = await api.pause(); show(added);
    if (added.graph.node?.filter((n) => n.folder === 'nodes/song-track').length !== 7) throw new Error('Add track did not create its player node');
    show(await api.undo());
    steps.push('zoom preserves absolute clip times and new tracks connect to the real song graph');

    await sleep(250);
    const track = [...panel()!.querySelectorAll('label')].find((l) => l.querySelector('span')?.textContent === 'Track')!.querySelector('select')!;
    track.value = '0'; track.dispatchEvent(new Event('change', { bubbles: true }));
    await sleep(250);

    click('arrange', 'Piano roll');
    await until(() => !!panel()?.querySelector('svg[aria-label^="Chromatic piano"]'), 'No chromatic piano roll');
    fit('arrange');
    await api.capture('song-piano');
    const piano = panel()!.querySelector<SVGSVGElement>('svg[aria-label^="Chromatic piano"]')!;
    piano.focus(); piano.dispatchEvent(new KeyboardEvent('keydown', { code: 'ArrowUp', key: 'ArrowUp', bubbles: true }));
    await sleep(350);
    const note = await api.pause(); show(note);
    if (!scoreText(note)?.includes('"key":63')) throw new Error('Piano pitch did not edit a real note');
    show(await api.undo());
    click('arrange', 'Mixer'); await sleep(250); fit('arrange'); await api.capture('song-mixer');
    click('arrange', 'Automation'); await sleep(250); fit('arrange');
    steps.push('the piano roll changes note pitch; mixer and automation controls fit the dock');
    click('arrange', 'Arrange'); await sleep(250);
    enterText(field('arrange', 'Audio clip / WAV'), 'types/synth-dsp/check.wav');
    await sleep(200); click('arrange', 'Import WAV clip'); await sleep(400);
    const imported = await api.pause(); show(imported);
    if (!scoreText(imported)?.includes('"kind":"Audio"')) throw new Error('WAV import did not add an audio clip');
    show(await api.save());
    show(await api.openGraph('main')); const reopened = await api.openGraph('song'); show(reopened);
    if (scoreText(reopened) !== scoreText(imported)) throw new Error('The imported clip did not survive save and reopen');
    steps.push('WAV import copies an audio asset and survives save and graph switching');
    await showTab('arrange'); await sleep(500); fit('arrange'); await api.capture('song-hybrid');
    show(await api.stop()); show(await api.openGraph('main'));
    return steps;
  } catch (error) {
    await api.capture('song-failed'); throw error;
  } finally { off(); }
}

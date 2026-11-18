// The widget steps of the self-check: each value widget of the GUI node reaches the node and
// comes back in its widget line (4.3).

import { createElement } from 'react';
import { createRoot } from 'react-dom/client';
import { RUN_MS, enterText, type CheckContext } from './check-fixtures';
import { FAR_ZOOM } from '../wires/gl';
import { Guard } from '../controls/NodeGui';

type Until = CheckContext['until'];

/** Uses each widget of the GUI node in the node body `gui`, and waits for the new value. */
export async function checkWidgets(until: Until, gui: string) {
  for (const kind of ['gui-color', 'gui-chart', 'gui-table', 'gui-curve', 'gui-image']) {
    if (!document.querySelector(`${gui} .${kind}`)) throw new Error(`the node GUI has no ${kind}`);
  }
  const row = () => document.querySelector<HTMLElement>(`${gui} .gui-table tbody tr:nth-child(2)`);
  row()!.click();
  await until('the selected row of the table', RUN_MS, () => row()?.getAttribute('aria-selected') === 'true');
  document.querySelector<HTMLButtonElement>(`${gui} button[aria-label="Increase Gain"]`)!.click();
  await until('the stepped number field', RUN_MS, () => document.querySelector<HTMLInputElement>(`${gui} input.gui-field`)?.value === '2');
  // A slider move shows its value and sends it at the pointer up. React shows the old value until the node answers.
  const slider = () => document.querySelector<HTMLInputElement>(`${gui} input.gui-slider`)!;
  Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!.call(slider(), '0.75');
  slider().dispatchEvent(new Event('input', { bubbles: true }));
  // A synthetic pointerup reaches the pane, which releases a pointer that does not exist. The slider also commits on keyup.
  slider().dispatchEvent(new KeyboardEvent('keyup', { bubbles: true }));
  await until('the slider value that the node keeps', RUN_MS, () => slider().value === '0.75' && slider().nextElementSibling?.textContent === '0.75');
  const mode = () => [...document.querySelectorAll<HTMLSelectElement>(`${gui} select.gui-field`)].find((s) => s.options[1]?.textContent === 'Hard')!;
  mode().value = '1';
  mode().dispatchEvent(new Event('change', { bubbles: true }));
  await until('the dropdown option that the node keeps', RUN_MS, () => mode().value === '1');
  const shapeTab = () => [...document.querySelectorAll<HTMLElement>(`${gui} [role="tab"]`)].find((t) => t.textContent === 'Shape');
  shapeTab()!.click();
  await until('the selected tab', RUN_MS, () => shapeTab()?.getAttribute('aria-selected') === 'true');
  document.querySelector<HTMLButtonElement>(`${gui} .gui-section-head`)!.click();
  await until('the open section', RUN_MS, () => document.querySelector(`${gui} .gui-section-body`)?.textContent === 'Hidden detail');
  const trimStart = () => document.querySelector<HTMLElement>(`${gui} [aria-label="Waveform trim start"]`);
  trimStart()!.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
  await until('the moved trim handle', RUN_MS, () => trimStart()?.getAttribute('aria-valuenow') === '0.05');
  const axisX = [...document.querySelectorAll(`${gui} .gui-axis-x span`)].map((s) => s.textContent).join(', ');
  if (axisX !== '0 ms, 99 ms, 0 s, 39 s') throw new Error(`the x axes of the plots show ${axisX}`);
  const dots = document.querySelector(`${gui} .dots`)?.getAttribute('d')?.split('M').length ?? 0;
  if (dots !== 41) throw new Error(`the scatter plot has ${dots - 1} dots, not 40`);
  if (!document.querySelector(`${gui} .gui-curve .histogram`)) throw new Error('the curve has no histogram');
  const rotate = () => document.querySelector<HTMLButtonElement>(`${gui} button[aria-label="Rotate right"]`);
  if (!rotate()?.querySelector('svg')) throw new Error('the icon button has no icon');
  rotate()!.click();
  await until('the progress bar after a click on the icon button', RUN_MS, () => document.querySelector(`${gui} [role="progressbar"]`)?.getAttribute('aria-valuenow') === '25');
  const stops = () => document.querySelectorAll<HTMLElement>(`${gui} .ramp-stop`);
  if (stops().length !== 3) throw new Error(`the color ramp has ${stops().length} stops, not 3`);
  const bar = document.querySelector<HTMLElement>(`${gui} .ramp-bar`)!.getBoundingClientRect();
  const quarter = { bubbles: true, button: 0, pointerId: 1, isPrimary: true, detail: 1, clientX: bar.left + bar.width / 4, clientY: bar.top + 5 };
  document.querySelector(`${gui} .ramp-bar`)!.dispatchEvent(new PointerEvent('pointerdown', quarter));
  await until('the stop that a click on the ramp adds', RUN_MS, () => stops().length === 4 && stops()[1].classList.contains('selected') && stops()[1].getAttribute('aria-valuenow') === '0.25');
  stops()[1].dispatchEvent(new KeyboardEvent('keydown', { key: 'Delete', bubbles: true }));
  await until('the ramp after a remove of the stop', RUN_MS, () => stops().length === 3);
  const blend = () => document.querySelector<HTMLSelectElement>(`${gui} select[aria-label="Ramp blend"]`)!;
  blend().value = 'constant';
  blend().dispatchEvent(new Event('change', { bubbles: true }));
  await until('the constant blend of the ramp', RUN_MS, () => blend().value === 'constant');
  const layers = () => document.querySelectorAll<HTMLElement>(`${gui} .gui-layer`);
  if (layers().length !== 3) throw new Error(`the layer stack has ${layers().length} rows, not 3`);
  layers()[0].querySelector<HTMLButtonElement>('.layer-eye')!.click();
  await until('the hidden layer', RUN_MS, () => layers()[0].querySelector('.layer-eye')?.getAttribute('aria-pressed') === 'false');
  const handle = layers()[0].querySelector<HTMLButtonElement>('.layer-handle')!;
  handle.focus();
  handle.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }));
  await until('the layer that the down arrow moves', RUN_MS, () => layers()[1].textContent?.includes('Edge polish') === true && layers()[1].classList.contains('selected'));
  // The dialog of the Browse button needs a person, so the check types the path (4.3).
  const pathField = () => document.querySelector(`${gui} [aria-label="Browse Path"]`)?.parentElement?.querySelector('input');
  enterText(pathField()!, 'out/b c.png');
  await until('the typed file path', RUN_MS, () => pathField()?.value === 'out/b c.png');
  // The asset picker lists the project files with its extension, and a pick goes to the node.
  // React puts the old value back after the change, so the new value shows only after the round trip.
  const source = () => document.querySelector<HTMLSelectElement>(`${gui} select:has(option[value="patina.toml"])`);
  await until('the project files in the asset picker', RUN_MS, () => !!source());
  source()!.value = 'patina.toml';
  source()!.dispatchEvent(new Event('change', { bubbles: true }));
  await until('the picked asset', RUN_MS, () => source()?.value === 'patina.toml');
  // The code field keeps line breaks, tabs and backslashes through the round trip to the node.
  const code = () => document.querySelector<HTMLTextAreaElement>(`${gui} textarea.gui-code`);
  if (code()?.value !== 'LDI 1\nOUT') throw new Error(`the code field shows ${JSON.stringify(code()?.value)}`);
  const program = 'LDI 2\n\tOUT ; a\\b';
  Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value')!.set!.call(code(), program);
  code()!.dispatchEvent(new Event('input', { bubbles: true }));
  code()!.dispatchEvent(new FocusEvent('focusout', { bubbles: true }));
  await until('the edited code field', RUN_MS, () => code()?.value === program);
  // A pointer down on the canvas goes to the node, and the node draws its dot at the point.
  const pad = () => document.querySelector<SVGSVGElement>(`${gui} svg.gui-canvas`);
  const dotX = () => Number(pad()?.querySelector('circle')?.getAttribute('cx'));
  if (dotX() !== 20) throw new Error(`the canvas dot is at x ${dotX()}, not 20`);
  const padBox = pad()!.getBoundingClientRect();
  const padAt = { bubbles: true, button: 0, pointerId: 1, isPrimary: true, clientX: padBox.left + padBox.width * 0.75, clientY: padBox.top + padBox.height / 2 };
  pad()!.dispatchEvent(new PointerEvent('pointerdown', padAt));
  pad()!.dispatchEvent(new PointerEvent('pointerup', padAt));
  await until('the canvas dot at the pointer', RUN_MS, () => Math.abs(dotX() - 120) < 1);
  // The canvas asks for keys, so the click gave it the focus, and a key down goes to the node.
  if (pad()?.getAttribute('role') !== 'application' || document.activeElement !== pad()) throw new Error('the key canvas did not take the focus');
  for (const type of ['keydown', 'keyup']) pad()!.dispatchEvent(new KeyboardEvent(type, { key: 'k', code: 'KeyK', bubbles: true }));
  await until('the canvas dot after the key K', RUN_MS, () => Math.abs(dotX() - 130) < 1);
  await checkDrags(until, gui);
}

/** A pointer event of the mouse at the client point `p`. */
const mouse = ([clientX, clientY]: number[]) => ({ bubbles: true, button: 0, pointerId: 1, isPrimary: true, clientX, clientY });
/** Lets React apply the state of the last event. */
const tick = () => new Promise<void>((r) => setTimeout(r));
const near = (text: string | null | undefined, v: number) => Math.abs(Number(text) - v) < 0.011;
const centre = (e: Element) => {
  const r = e.getBoundingClientRect();
  return [r.left + r.width / 2, r.top + r.height / 2];
};

/** Drags a curve point, a trim handle, a ramp stop, a layer, a canvas, a gizmo and a clip with pointer events (4.3). Each widget drag sends one change at its end. */
async function checkDrags(until: Until, gui: string) {
  const points = () => document.querySelectorAll<SVGCircleElement>(`${gui} .gui-curve circle`);
  const curve = document.querySelector(`${gui} svg.gui-curve`)!;
  // The points start at (0, 0.2), (0.5, 0.8) and (1, 0.4).
  const [[x0, y0], [, y8], [x1]] = [...points()].map(centre);
  const curveAt = (x: number, y: number) => mouse([x0 + x * (x1 - x0), y0 + ((y - 0.2) / 0.6) * (y8 - y0)]);
  const point = () => points()[1].getAttribute('aria-label')?.match(/x (\S+), y (\S+)$/);
  points()[1].dispatchEvent(new PointerEvent('pointerdown', curveAt(0.5, 0.8)));
  await until('the curve point that a pointer down selects', RUN_MS, () => points()[1].classList.contains('selected'));
  curve.dispatchEvent(new PointerEvent('pointermove', curveAt(0.6, 0.5)));
  await until('the curve point during a drag', RUN_MS, () => near(point()?.[1], 0.6) && near(point()?.[2], 0.5));
  curve.dispatchEvent(new PointerEvent('pointerup', curveAt(0.6, 0.5)));
  await tick();
  await until('the dragged curve point', RUN_MS, () => near(point()?.[1], 0.6) && near(point()?.[2], 0.5));

  const area = document.querySelector<HTMLElement>(`${gui} .trim-area`)!;
  const box = area.getBoundingClientRect();
  const trimAt = (x: number) => mouse([box.left + x * box.width, box.top + box.height / 2]);
  const trimEnd = () => document.querySelector(`${gui} [aria-label="Waveform trim end"]`)?.getAttribute('aria-valuenow');
  area.dispatchEvent(new PointerEvent('pointerdown', trimAt(0.98)));
  await until('the trim end at the pointer down', RUN_MS, () => near(trimEnd(), 0.98));
  area.dispatchEvent(new PointerEvent('pointermove', trimAt(0.75)));
  await until('the trim end during a drag', RUN_MS, () => near(trimEnd(), 0.75));
  area.dispatchEvent(new PointerEvent('pointerup', trimAt(0.75)));
  await tick();
  await until('the dragged trim end', RUN_MS, () => near(trimEnd(), 0.75));

  const ramp = document.querySelector<HTMLElement>(`${gui} .gui-ramp`)!;
  const bar = ramp.getBoundingClientRect();
  const rampAt = (x: number) => mouse([bar.left + x * bar.width, bar.top + 5]);
  const stop = () => document.querySelectorAll(`${gui} .ramp-stop`)[1]?.getAttribute('aria-valuenow');
  document.querySelectorAll(`${gui} .ramp-stop`)[1].dispatchEvent(new PointerEvent('pointerdown', rampAt(0.62)));
  await tick();
  ramp.dispatchEvent(new PointerEvent('pointermove', rampAt(0.4)));
  await until('the ramp stop during a drag', RUN_MS, () => near(stop(), 0.4));
  ramp.dispatchEvent(new PointerEvent('pointerup', rampAt(0.4)));
  await tick();
  await until('the dragged ramp stop', RUN_MS, () => near(stop(), 0.4));

  // The rows are Verdigris, Edge polish and Copper. The drag takes Verdigris to the bottom, past two rows.
  const layers = () => document.querySelectorAll<HTMLElement>(`${gui} .gui-layer`);
  const rows = [...layers()].map(centre);
  const grip = layers()[0].querySelector<HTMLButtonElement>('.layer-handle')!;
  grip.dispatchEvent(new PointerEvent('pointerdown', mouse(rows[0])));
  await until('the layer that a drag holds', RUN_MS, () => layers()[0].classList.contains('dragged'));
  for (const i of [1, 2]) {
    grip.dispatchEvent(new PointerEvent('pointermove', mouse(rows[i])));
    await until(`the dragged layer in row ${i + 1}`, RUN_MS, () => layers()[i].classList.contains('dragged'));
  }
  grip.dispatchEvent(new PointerEvent('pointerup', mouse(rows[2])));
  await tick();
  await until('the layer that a drag moves to the bottom', RUN_MS, () => !!layers()[2].textContent?.includes('Verdigris') && !layers()[2].classList.contains('dragged'));

  // The canvas sends the moves of a drag before the up, and the node draws its dot at each move.
  const pad = document.querySelector<SVGSVGElement>(`${gui} svg.gui-canvas`)!;
  const dotX = () => Number(pad.querySelector('circle')?.getAttribute('cx'));
  const padBox = pad.getBoundingClientRect();
  const padAt = (x: number) => mouse([padBox.left + x * padBox.width, padBox.top + padBox.height / 2]);
  pad.dispatchEvent(new PointerEvent('pointerdown', padAt(0.25)));
  await until('the canvas dot at the pointer down', RUN_MS, () => Math.abs(dotX() - 40) < 1);
  pad.dispatchEvent(new PointerEvent('pointermove', padAt(0.5)));
  await until('the canvas dot during a drag', RUN_MS, () => Math.abs(dotX() - 80) < 1);
  pad.dispatchEvent(new PointerEvent('pointerup', padAt(0.5)));
  await tick();

  // A drag of the transform box by a quarter of the gizmo is 50 of its 200 units: 48.78 px of the 160 px frame.
  const gizmo = document.querySelector<SVGSVGElement>(`${gui} svg.gui-transform`)!;
  if (!gizmo.querySelector('image')?.getAttribute('href')?.startsWith('data:image/png')) throw new Error('the transform gizmo shows no image');
  const moveX = () => Number(document.querySelector<HTMLInputElement>(`${gui} .gui-transform-fields input`)?.value);
  const [bx, by] = centre(gizmo.querySelector('.box')!);
  const to = mouse([bx + gizmo.getBoundingClientRect().width / 4, by]);
  gizmo.querySelector('.box')!.dispatchEvent(new PointerEvent('pointerdown', mouse([bx, by])));
  await tick();
  gizmo.dispatchEvent(new PointerEvent('pointermove', to));
  await until('the transform X during a drag', RUN_MS, () => Math.abs(moveX() - 48.78) < 0.01);
  gizmo.dispatchEvent(new PointerEvent('pointerup', to));
  await tick();
  await until('the transform X that the node keeps', RUN_MS, () => Math.abs(moveX() - 48.78) < 0.01 && !gizmo.querySelector('.dragging'));
  // The pivot buttons and Aspect lock write the transform that the node keeps. A pivot change keeps the image in place.
  const pivot = (label: string) => document.querySelector<HTMLButtonElement>(`${gui} .gui-pivots button[aria-label="${label}"]`)!;
  const lockBox = () => document.querySelector<HTMLInputElement>(`${gui} .gui-transform-fields input[type="checkbox"]`)!;
  for (const p of ['Top left', 'Center']) {
    pivot(p).click();
    await until(`the ${p} pivot that the node keeps`, RUN_MS, () => pivot(p).getAttribute('aria-checked') === 'true' && Math.abs(moveX() - 48.78) < 0.01);
  }
  for (const on of [false, true]) {
    lockBox().click();
    await until(`the aspect lock ${on ? 'on' : 'off'} that the node keeps`, RUN_MS, () => lockBox().checked === on);
  }

  // A drag of the clip by a third of its lane is 1 s of the 3 s timeline, on the snap of 0.1 s.
  const timeline = document.querySelector<HTMLElement>(`${gui} .gui-timeline`)!;
  const clipLabel = () => timeline.querySelector('.tl-clip')?.getAttribute('aria-label');
  const lane = timeline.querySelector<HTMLElement>('.tl-track .tl-area')!.getBoundingClientRect();
  const [cx, cy] = centre(timeline.querySelector('.tl-clip')!);
  timeline.querySelector('.tl-clip')!.dispatchEvent(new PointerEvent('pointerdown', mouse([cx, cy])));
  await tick();
  timeline.dispatchEvent(new PointerEvent('pointermove', mouse([cx + lane.width / 3, cy])));
  await until('the clip start during a drag', RUN_MS, () => clipLabel() === 'Hit: start 1 s, length 1 s');
  timeline.dispatchEvent(new PointerEvent('pointerup', mouse([cx + lane.width / 3, cy])));
  await tick();
  await until('the clip start that the node keeps', RUN_MS, () => clipLabel() === 'Hit: start 1 s, length 1 s' && !!timeline.querySelector('.tl-clip.selected'));
  // The transport row writes the play, loop and snap state that the node keeps. React puts the old value back until the node answers.
  const transport = (label: string) => timeline.querySelector<HTMLButtonElement>(`.tl-transport button[aria-label="${label}"]`)!;
  transport('Play').click();
  await until('the play state that the node keeps', RUN_MS, () => transport('Play').getAttribute('aria-pressed') === 'true');
  transport('Pause').click();
  await until('the pause state that the node keeps', RUN_MS, () => transport('Play').getAttribute('aria-pressed') === 'false');
  const loopBox = timeline.querySelector<HTMLInputElement>('.tl-option input[type="checkbox"]')!;
  loopBox.click();
  await until('the loop that the node keeps', RUN_MS, () => loopBox.checked && !!timeline.querySelector('.tl-loop.on'));
  const snapList = timeline.querySelector<HTMLSelectElement>('.tl-option select')!;
  snapList.value = '0.5';
  snapList.dispatchEvent(new Event('change', { bubbles: true }));
  await until('the snap that the node keeps', RUN_MS, () => snapList.value === '0.5');
  const playhead = timeline.querySelector<HTMLElement>('[role="slider"][aria-label="Playhead"]')!;
  playhead.dispatchEvent(new KeyboardEvent('keydown', { key: 'End', bubbles: true, cancelable: true }));
  await until('the playhead at the end after the End key', RUN_MS, () => Number(playhead.getAttribute('aria-valuenow')) === Number(playhead.getAttribute('aria-valuemax')));
  transport('Rewind').click();
  await until('the playhead at 0 after Rewind', RUN_MS, () => playhead.getAttribute('aria-valuenow') === '0');
  // M and S of a lane write the mute and the solo that the node keeps. A muted lane is quiet.
  const ms = (what: string) => timeline.querySelector<HTMLButtonElement>(`.tl-ms[aria-label^="${what} "]`)!;
  const pressed = (what: string) => ms(what).getAttribute('aria-pressed') === 'true';
  ms('Mute').click();
  await until('the mute that the node keeps', RUN_MS, () => pressed('Mute') && !!timeline.querySelector('.tl-head.quiet'));
  ms('Mute').click();
  await until('the lane without mute', RUN_MS, () => !pressed('Mute') && !timeline.querySelector('.tl-head.quiet'));
  ms('Solo').click();
  await until('the solo that the node keeps', RUN_MS, () => pressed('Solo'));
  ms('Solo').click();
  await until('the lane without solo', RUN_MS, () => !pressed('Solo'));
  // A double click on an automation lane adds a point, and the Delete key on a point removes it.
  const keys = () => timeline.querySelectorAll<HTMLElement>('.tl-point');
  const auto = timeline.querySelector<HTMLElement>('.tl-area[title="Double click to add a point"]')!;
  const laneBox = auto.getBoundingClientRect();
  auto.dispatchEvent(new MouseEvent('dblclick', { bubbles: true, clientX: laneBox.left + laneBox.width * 0.8, clientY: laneBox.top + laneBox.height / 2 }));
  await until('a second point on the automation lane', RUN_MS, () => keys().length === 2);
  keys()[1].dispatchEvent(new KeyboardEvent('keydown', { key: 'Delete', bubbles: true }));
  await until('one point after the Delete key', RUN_MS, () => keys().length === 1);
}

/** A GUI that throws shows its error in its own box, and its next tree draws again (16.1). */
export async function checkGuard(until: Until) {
  const box = document.createElement('div');
  document.body.append(box);
  const root = createRoot(box);
  const Throw = () => {
    throw new Error('a test failure');
  };
  try {
    root.render(createElement(Guard, { tree: 'a', label: 'Test GUI', children: createElement(Throw) }));
    await until('the error of a GUI that throws', RUN_MS, () => box.querySelector('[role="alert"]')?.textContent?.includes('a test failure') ?? false);
    root.render(createElement(Guard, { tree: 'b', label: 'Test GUI', children: 'drawn' }));
    await until('the next tree of a GUI that threw', RUN_MS, () => box.textContent === 'drawn');
  } finally {
    root.unmount();
    box.remove();
  }
}

/** The node GUI `gui` and the rows draw down to FAR_ZOOM, and both hide below it at the same zoom (15.1). The zoom comes back. */
export async function checkGuiZoom(until: Until, gui: string, wheel: (deltaY: number) => void, scale: () => number) {
  const node = () => document.querySelector(gui)!.closest('.patina-node')!;
  const shown = () => getComputedStyle(document.querySelector(gui)!).visibility !== 'hidden';
  let outs = 0;
  while (scale() >= FAR_ZOOM) {
    if (node().classList.contains('far') || !shown()) throw new Error(`the node rows or GUI do not draw at the zoom ${scale()}`);
    const z = scale();
    wheel(240);
    outs++;
    await until('a smaller zoom', RUN_MS, () => scale() < z);
  }
  await until('the simple node form without the GUI', RUN_MS, () => node().classList.contains('far') && !shown());
  for (; outs > 0; outs--) {
    const z = scale();
    wheel(-240);
    await until('a larger zoom', RUN_MS, () => scale() > z);
  }
  await until('the node GUI after the zoom', RUN_MS, shown);
}

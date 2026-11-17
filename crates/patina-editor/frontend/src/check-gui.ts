// The widget steps of the self-check: each value widget of the GUI node reaches the node and
// comes back in its widget line (4.3).

import { createElement } from 'react';
import { createRoot } from 'react-dom/client';
import { RUN_MS, enterText } from './check-fixtures';
import { Guard } from './NodeGui';

type Until = (what: string, ms: number, test: () => boolean) => Promise<void>;

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

import { PITCH } from '../canvas/bus';

/** The lanes of a wire, read from its hit path (the width of a ribbon) or its label ("/ n"), else 0. */
function lanesOf(edge: Element, path: SVGPathElement): number {
  const px = /(\d+(?:\.\d+)?)px\)?$/.exec(path.style.strokeWidth)?.[1];
  const fromStroke = px === undefined ? 0 : Math.round((Number(px) - 4) / PITCH);
  if (fromStroke > 1) return fromStroke;
  const id = edge.getAttribute('data-id');
  const label = [...document.querySelectorAll<HTMLElement>('.wire-type')].find((l) => l.dataset.wire === id);
  return Number(/\/ (\d+)$/.exec(label?.textContent ?? '')?.[1] ?? 0);
}

/** One fault for each end of a wire of more than 1 lane that is not a `.bus-pin` bar as high as the ribbon (15.9). */
export function busPinFaults(): string[] {
  const bad: string[] = [];
  for (const e of document.querySelectorAll('.react-flow__edge')) {
    const id = e.getAttribute('data-id') ?? '';
    const path = e.querySelector<SVGPathElement>('.wire-hit');
    const m = path?.getScreenCTM();
    if (!path || !m || !id.includes('>')) continue;
    const handles = id.split('>').map((pin) => {
      const dot = pin.lastIndexOf('.');
      return document.querySelector<HTMLElement>(`.react-flow__node[data-id="${pin.slice(0, dot)}"] .react-flow__handle[data-handleid="${pin.slice(dot + 1)}"]`);
    });
    const bar = (h: HTMLElement | null) => (h?.classList.contains('bus-pin') ? Math.round((h.getBoundingClientRect().height / m.a - 4) / PITCH) : 0);
    // A bus-pin at one end also shows that the wire is a bus.
    const n = lanesOf(e, path) || Math.max(...handles.map(bar));
    if (n <= 1) continue;
    id.split('>').forEach((pin, i) => {
      const h = handles[i];
      if (!h) return;
      if (!h.classList.contains('bus-pin')) bad.push(`the bus wire ${id} of ${n} lanes ends in the dot pin ${pin}`);
      else if (h.getBoundingClientRect().height / m.a < n * PITCH - 0.5) bad.push(`the bus wire ${id} of ${n} lanes ends in the short bus pin ${pin}`);
    });
  }
  return bad;
}

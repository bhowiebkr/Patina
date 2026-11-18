// The text contrast check of the shot (15.3): WCAG 2 asks 4.5:1 for small text and 3:1 for large text.

type Rgba = [number, number, number, number];

/** The channels of a computed CSS color `rgb(...)` or `rgba(...)`, or undefined for another form. */
export function parseColor(css: string): Rgba | undefined {
  const m = /^rgba?\(([\d.]+),\s*([\d.]+),\s*([\d.]+)(?:,\s*([\d.]+))?\)$/.exec(css.trim());
  return m ? [+m[1], +m[2], +m[3], m[4] === undefined ? 1 : +m[4]] : undefined;
}

/** `top` drawn over the opaque color `under`. */
export function blend(top: Rgba, under: Rgba): Rgba {
  const a = top[3];
  return [0, 1, 2].map((i) => top[i] * a + under[i] * (1 - a)).concat(1) as Rgba;
}

const luminance = (c: Rgba) => {
  const [r, g, b] = c.slice(0, 3).map((v) => (v / 255 <= 0.04045 ? v / 255 / 12.92 : ((v / 255 + 0.055) / 1.055) ** 2.4));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
};

/** The WCAG 2 contrast ratio of two opaque colors, from 1 to 21. */
export function ratio(a: Rgba, b: Rgba) {
  const [x, y] = [luminance(a), luminance(b)].sort((p, q) => q - p);
  return (x + 0.05) / (y + 0.05);
}

/** The opaque color under `e`: the backgrounds of its parents, blended down to the page. Undefined over an image, a canvas or an SVG. */
function background(e: Element): Rgba | undefined {
  const layers: Rgba[] = [];
  for (let a: Element | null = e; a; a = a.parentElement) {
    const s = getComputedStyle(a);
    if (s.backgroundImage !== 'none') return undefined;
    const c = parseColor(s.backgroundColor);
    if (c && c[3] > 0) layers.push(c);
    if (c && c[3] === 1) break;
  }
  let under: Rgba = [0, 0, 0, 1];
  for (const c of layers.reverse()) under = blend(c, under);
  return under;
}

/** The texts in `root` with too little contrast. A disabled control, a text in an SVG and a text over an image are out of the check (WCAG 1.4.3). */
export function lowContrast(root: ParentNode) {
  const bad: string[] = [];
  let seen = 0;
  const walk = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  const done = new Set<Element>();
  for (let t = walk.nextNode(); t; t = walk.nextNode()) {
    const e = t.parentElement;
    if (!e || done.has(e) || !t.textContent?.trim() || e.closest('svg, .monaco-editor, :disabled, [aria-disabled=true]')) continue;
    done.add(e);
    const r = e.getBoundingClientRect();
    const s = getComputedStyle(e);
    if (r.width === 0 || s.visibility === 'hidden') continue;
    const under = background(e);
    const color = parseColor(s.color);
    if (!under || !color) continue;
    let alpha = color[3];
    for (let a: Element | null = e; a; a = a.parentElement) alpha *= +getComputedStyle(a).opacity;
    // A caption that shows only on hover has an opacity of 0.
    if (alpha === 0) continue;
    seen++;
    const size = parseFloat(s.fontSize);
    const large = size >= 24 || (size >= 18.66 && +s.fontWeight >= 700);
    const got = ratio(blend([color[0], color[1], color[2], alpha], under), under);
    if (got < (large ? 3 : 4.5)) bad.push(`"${t.textContent.trim().slice(0, 24)}" ${got.toFixed(2)}:1 in ${e.closest('[data-id], [aria-label]')?.getAttribute('data-id') ?? e.closest('[aria-label]')?.getAttribute('aria-label') ?? 'the page'}`);
  }
  return { bad, seen };
}

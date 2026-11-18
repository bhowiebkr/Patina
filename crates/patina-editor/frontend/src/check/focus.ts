// The keyboard focus check of the shot (WCAG 2.4.7). A hidden window never matches :focus, so the check reads the style rules.

const FOCUS = /:focus(?:-visible|-within)?(?![\w-])/;

/** The parts of a selector list, split at the commas outside of brackets. */
export function splitSelectors(list: string) {
  const parts: string[] = [];
  let [depth, start] = [0, 0];
  for (let i = 0; i < list.length; i++) {
    const c = list[i];
    if (c === '(' || c === '[') depth++;
    else if (c === ')' || c === ']') depth--;
    else if (c === ',' && depth === 0) {
      parts.push(list.slice(start, i).trim());
      start = i + 1;
    }
  }
  parts.push(list.slice(start).trim());
  return parts;
}

/**
 * The selector of the element that gets the focus in `selector`, without the focus pseudo-classes and pseudo-elements, or undefined with no focus pseudo-class.
 * `near` is true when the focus can be on a child of that element: `:focus-within` or a focus in brackets, for example `:has(...)`.
 */
export function focusTarget(selector: string): { target: string; near: boolean } | undefined {
  const m = FOCUS.exec(selector);
  if (!m) return undefined;
  let depth = 0;
  for (const c of selector.slice(0, m.index)) depth += c === '(' || c === '[' ? 1 : c === ')' || c === ']' ? -1 : 0;
  // The compound with the focus ends at the next combinator outside of brackets.
  let end = m.index;
  for (let d = depth; end < selector.length; end++) {
    const c = selector[end];
    if (c === '(' || c === '[') d++;
    else if (c === ')' || c === ']') d--;
    else if (d < 0 || (d === 0 && /[\s>+~]/.test(c))) break;
  }
  if (depth > 0) end = selector.length;
  const target = selector
    .slice(0, end)
    .replace(/(^|[\s>+~(])(?=:focus)/g, '$1*')
    .replace(new RegExp(FOCUS.source, 'g'), '')
    .replace(/::[\w-]+/g, '')
    .trim();
  return { target: target || '*', near: depth > 0 || m[0] === ':focus-within' };
}

const hides = (s: CSSStyleDeclaration) => s.outlineStyle === 'none' || /^0(px)?$/.test(s.outlineWidth);

/** The buttons, fields and tabs in `root` that a keyboard focus does not mark: no focus rule changes them, and a rule hides the outline of the browser. */
export function noFocusLook(root: ParentNode) {
  const rules: CSSStyleRule[] = [];
  const walk = (list: CSSRuleList) => {
    for (const r of list) {
      if (r instanceof CSSStyleRule) rules.push(r);
      else if ('cssRules' in r) walk((r as CSSGroupingRule).cssRules);
    }
  };
  for (const sheet of document.styleSheets) {
    try {
      walk(sheet.cssRules);
    } catch {
      // A sheet of another origin cannot be read.
    }
  }
  const looks: { target: string; near: boolean }[] = [];
  const hiders: string[] = [];
  for (const r of rules)
    for (const s of splitSelectors(r.selectorText)) {
      const f = focusTarget(s);
      // A rule that only hides the outline is no focus look.
      const onlyHides = hides(r.style) && Array.from({ length: r.style.length }, (_, i) => r.style[i]).every((p) => p.startsWith('outline'));
      if (f && !onlyHides) looks.push(f);
      if (hides(r.style)) hiders.push(f?.target ?? s);
    }
  const matches = (w: Element, s: string, near = false) => {
    try {
      return near ? !!w.closest(s) : w.matches(s);
    } catch {
      return false;
    }
  };
  const bad: Element[] = [];
  let seen = 0;
  for (const w of root.querySelectorAll<HTMLElement>('button, input, select, textarea, [role=tab], [tabindex="0"]')) {
    if (!(w instanceof HTMLElement) || w.closest('.monaco-editor') || w.matches(':disabled') || w.tabIndex < 0 || w.getBoundingClientRect().width === 0) continue;
    seen++;
    if (!looks.some((l) => matches(w, l.target, l.near)) && hiders.some((h) => matches(w, h))) bad.push(w);
  }
  return { bad, seen };
}

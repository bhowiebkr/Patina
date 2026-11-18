// The click coverage of the self-check: the buttons, menu items and tabs that showed but got no click.

const CONTROLS = 'button, [role=menuitem], [role=tab]';

/** The name of a control as the self-check finds it: the label, or the first text of a menu item. */
export function controlName(c: Element) {
  const text = c.getAttribute('role') === 'menuitem' ? c.childNodes[0]?.textContent : c.textContent;
  return (c.getAttribute('aria-label') ?? text ?? '').trim().replace(/\s+/g, ' ').slice(0, 40);
}

/** Records each control that shows and each control that gets a click, until `stop`. The node GUIs and the code editor are out. */
export function watchClicks() {
  const seen = new Set<string>();
  const clicked = new Set<string>();
  const out = (c: Element) => !!c.closest('.react-flow__node, .node-gui, .monaco-editor');
  const onClick = (e: Event) => {
    const c = e.target instanceof Element ? e.target.closest(CONTROLS) : null;
    if (c && !out(c)) clicked.add(controlName(c));
  };
  let timer: ReturnType<typeof setTimeout> | undefined;
  const sample = () => {
    timer = undefined;
    for (const c of document.querySelectorAll(CONTROLS)) if (!out(c) && c.getBoundingClientRect().width > 0) seen.add(controlName(c));
  };
  // A sample at most each 50 ms keeps the check fast, and still sees a menu that the check opens.
  const watch = new MutationObserver(() => (timer ??= setTimeout(sample, 50)));
  document.addEventListener('click', onClick, true);
  watch.observe(document.body, { childList: true, subtree: true });
  sample();
  return () => {
    watch.disconnect();
    clearTimeout(timer);
    document.removeEventListener('click', onClick, true);
    const missed = [...seen].filter((n) => n && !clicked.has(n)).sort();
    return `clicks: ${clicked.size} of ${seen.size} controls, no click on ${missed.length}: ${missed.join(' | ')}`;
  };
}

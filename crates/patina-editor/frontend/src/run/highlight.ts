// The highlight of the Logs and Output lines (15.8): the parts of compiler and run text get colors,
// as in a terminal. Each part is a span with a class `hl-<kind>`.

export type Part = [text: string, kind: string | null];

const PART = new RegExp(
  [
    String.raw`(?<sev>\b(?:error|warning|note|help)(?:\[[A-Z]\d{4}\])?(?=:)|\b(?:ERROR|WARN|INFO|DEBUG|TRACE)\b)`,
    String.raw`(?<fix>^Fix:)`,
    String.raw`(?<path>(?:[A-Za-z]:)?(?:[\w.-]+[\\/])*[\w.-]+\.(?:rs|toml|ts|tsx|wgsl)(?::\d+){0,2})`,
    String.raw`(?<gutter>^\s*\d*\s*\|(?=\s|$)|^\s*-->|^\s*= (?=note|help))`,
    String.raw`(?<mark>\^+)`,
    '(?<code>`[^`\\n]+`)',
    String.raw`(?<str>"(?:[^"\\\n]|\\.)*")`,
    String.raw`(?<id>\b[0-9a-f]{16}\b)`,
    String.raw`(?<num>\b\d+(?:\.\d+)?(?: ?(?:ms|us|s|KB|MB|GB|Hz|frames?)\b|%)?)`,
  ].join('|'),
  'gm',
);

/** The parts of a line. A part with no kind has the color of the line. */
export function highlight(text: string): Part[] {
  const parts: Part[] = [];
  let at = 0;
  for (const m of text.matchAll(PART)) {
    const kind = Object.entries(m.groups ?? {}).find(([, v]) => v !== undefined)?.[0];
    if (!kind || m[0] === '') continue;
    if (m.index > at) parts.push([text.slice(at, m.index), null]);
    // The severity word gives its own color: error, warning, or note and help.
    const word = m[0].toLowerCase();
    parts.push([m[0], kind === 'sev' ? `sev-${word.startsWith('err') ? 'err' : word.startsWith('warn') ? 'warn' : 'note'}` : kind]);
    at = m.index + m[0].length;
  }
  if (at < text.length) parts.push([text.slice(at), null]);
  return parts;
}

// A compare of the canvas data, so a run frame renders only the nodes that changed (15.2).

/** True when two values are equal. Arrays and objects compare by their items, down to `depth` levels. */
export function same(a: unknown, b: unknown, depth: number): boolean {
  if (a === b) return true;
  if (depth === 0 || typeof a !== 'object' || typeof b !== 'object' || a === null || b === null) return false;
  const [x, y] = [a as Record<string, unknown>, b as Record<string, unknown>];
  const keys = Object.keys(x);
  return keys.length === Object.keys(y).length && keys.every((k) => k in y && same(x[k], y[k], depth - 1));
}

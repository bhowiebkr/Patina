// The search of the Add node palette (spec 15.2).

/** The nodes whose name contains `query`, with no regard to case. A name that starts with it comes first. */
export function matchNodes<T extends { name: string }>(nodes: readonly T[], query: string): T[] {
  const q = query.trim().toLowerCase();
  const late = (n: T) => Number(!n.name.toLowerCase().startsWith(q));
  return nodes.filter((n) => n.name.toLowerCase().includes(q)).sort((a, b) => late(a) - late(b) || a.name.localeCompare(b.name));
}

/** The type that a wire carries to or from a pin: no `&` or `&mut`, and `String` for `&str` (5.1). */
export function wireType(ty: string): string {
  const base = ty.replace(/^&('\w+\s+)?(mut\s+)?/, '').trim();
  return base === 'str' ? 'String' : base;
}

interface Pins {
  inputs: readonly { key: string; ty: string }[];
  outputs: readonly { key: string; ty: string }[];
}

/** The key of the first pin of `node` that a wire can join to a pin of type `ty`, an output when `output`. */
export function matchPin(node: Pins, output: boolean, ty: string): string | undefined {
  // shortcut: compares the type text, so two paths of one type do not match; rustc checks at build time.
  return (output ? node.inputs : node.outputs).find((p) => wireType(p.ty) === wireType(ty))?.key;
}

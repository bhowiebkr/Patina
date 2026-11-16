// The search of the Add node palette (spec 15.2).

/**
 * The nodes whose name contains `query`, with no regard to case. A name that starts with it comes
 * first, and the nodes of the project come before the built-in nodes.
 */
export function matchNodes<T extends { name: string; builtin?: boolean }>(nodes: readonly T[], query: string): T[] {
  const q = query.trim().toLowerCase();
  const late = (n: T) => Number(!n.name.toLowerCase().startsWith(q)) * 2 + Number(!!n.builtin);
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
  generics: readonly string[];
}

/**
 * The key of the first pin of `node` that a wire can join to a pin of type `ty`, an output when
 * `output`. A pin whose type is a type parameter of the node joins all types (4.5).
 */
export function matchPin(node: Pins, output: boolean, ty: string): string | undefined {
  // shortcut: compares the type text, so two paths of one type do not match; rustc checks at build time.
  const fits = (pinTy: string) => node.generics.includes(pinTy) || wireType(pinTy) === wireType(ty);
  return (output ? node.inputs : node.outputs).find((p) => fits(p.ty))?.key;
}

/**
 * The Rust type of output `from`, "<node id>.<output key>". A generic output, as of a delay, has the
 * type of the wire into an input of the same type (4.5). `wireTo` gives the source of the wire to an input.
 */
export function outputTypeOf(from: string, pins: (id: string) => Pins | null | undefined, wireTo: (to: string) => string | undefined, seen: string[] = []): string | undefined {
  const dot = from.lastIndexOf('.');
  const id = from.slice(0, dot);
  const node = pins(id);
  const ty = node?.outputs.find((p) => p.key === from.slice(dot + 1))?.ty;
  if (ty === undefined || !node!.generics.includes(ty) || seen.includes(from)) return ty;
  const input = node!.inputs.find((p) => p.ty === ty);
  const source = input && wireTo(`${id}.${input.key}`);
  const found = source === undefined ? undefined : outputTypeOf(source, pins, wireTo, [...seen, from]);
  return found ?? ty;
}

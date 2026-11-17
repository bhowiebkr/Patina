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

/** The wire type with no spaces and no module paths, so `Vec<std::string::String>` is `Vec<String>`. */
function typeKey(ty: string): string {
  return wireType(ty).replace(/\s+/g, '').replace(/(\b\w+)?::/g, '');
}

interface Pins {
  inputs: readonly { key: string; ty: string }[];
  outputs: readonly { key: string; ty: string }[];
  generics: readonly string[];
}

/** The type key of a socket, or '*' for a type parameter of the node, which joins all types (4.5). */
export function socketKey(ty: string, generics: readonly string[]): string {
  return generics.includes(ty) ? '*' : typeKey(ty);
}

/** The key of the first pin of `node` that a wire can join to a pin of type `ty`, an output when `output`. */
export function matchPin(node: Pins, output: boolean, ty: string): string | undefined {
  // shortcut: drops the module paths, so two types of one name in two modules match; rustc checks at build time.
  const fits = (pinTy: string) => [typeKey(ty), '*'].includes(socketKey(pinTy, node.generics));
  return (output ? node.inputs : node.outputs).find((p) => fits(p.ty))?.key;
}

/**
 * The CSS of a wire drag from a `side` socket with the type key `key`, or 'order' (15.9). The
 * sockets of the other side that fit get an accent ring, and all other sockets dim.
 */
export function dragStyle(side: 'source' | 'target', key: string): string {
  const h = `.react-flow .react-flow__handle.${side === 'source' ? 'target' : 'source'}`;
  const quote = (s: string) => `"${s.replace(/["\\]/g, '\\$&')}"`;
  const fit = key === 'order' ? `${h}.order-pin` : `${h}:not(.order-pin)${key === '*' ? '' : `:is([data-ty="*"], [data-ty=${quote(key)}])`}`;
  // The order socket is a diamond of gradients, so its edge takes the accent in place of a ring.
  const ring = key === 'order' ? '--diamond: var(--accent)' : 'box-shadow: 0 0 0 2px var(--accent)';
  return `.react-flow .react-flow__handle:not(.connectingfrom) { opacity: 0.3; }\n${fit} { opacity: 1; ${ring}; }`;
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

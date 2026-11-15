// The search of the Add node palette (spec 15.2).

/** The nodes whose name contains `query`, with no regard to case. A name that starts with it comes first. */
export function matchNodes<T extends { name: string }>(nodes: readonly T[], query: string): T[] {
  const q = query.trim().toLowerCase();
  const late = (n: T) => Number(!n.name.toLowerCase().startsWith(q));
  return nodes.filter((n) => n.name.toLowerCase().includes(q)).sort((a, b) => late(a) - late(b) || a.name.localeCompare(b.name));
}

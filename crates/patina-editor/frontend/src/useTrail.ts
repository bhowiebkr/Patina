// The trail of the open graphs, and the use path of the open graph (15.9).
import { useCallback, useMemo, useState, type RefObject } from 'react';
import { api, scopeDomains, type View } from './api';
import { crumbLabel, trailOf, usePathOf, type Crumb } from './crumbs';
import { scopeRun, TEST_USE, type RunState } from './NodeView';

export function useTrail(view: View | null, current: RefObject<View | null>, run: RunState, act: (job: Promise<View>, after?: (v: View) => void) => Promise<void>) {
  // A change of graph keeps the unsaved changes of each graph in memory. Only Save writes files (15.10).
  // A use opens the subgraph with the live values of that use. The path bar keeps each use (15.9).
  const [trail, setTrail] = useState<Crumb[]>([]);
  const goTo = useCallback(
    async (name: string, next: (trail: Crumb[]) => Crumb[]) => {
      setTrail(next);
      let opened: View | undefined;
      await act(api.openGraph(name), (v) => (opened = v));
      return opened;
    },
    [act],
  );
  const openGraph = useCallback(
    (name: string, use?: string) => {
      const at = current.current;
      if (use === undefined || !at) return goTo(name, () => trailOf([], name));
      const crumb = { graph: name, use, label: crumbLabel(at.graph.node ?? [], use) };
      return goTo(name, (t) => [...trailOf(t, at.graphName), crumb]);
    },
    [goTo, current],
  );
  const path = view ? trailOf(trail, view.graphName) : [];
  const testing = view?.test != null;
  // The run IDs of the open graph start with this use path (6.4). A subgraph with no use has none.
  const usePath = view?.graphName === 'main' ? '' : testing ? TEST_USE : usePathOf(path, view?.uses ?? []);
  const scoped = useMemo(() => scopeRun(run, usePath), [run, usePath]);
  // A subgraph with no use has no checked nodes, so its nodes show no domain.
  const domains = useMemo(() => scopeDomains(view && (view.graphName === 'main' || usePath) ? view.domains : {}, usePath), [view, usePath]);
  return { path, testing, usePath, scoped, domains, goTo, openGraph };
}

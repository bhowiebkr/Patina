import { createRoot } from 'react-dom/client';
import { App, reportError } from './App';
import type { Config, FeedMode, Graph, Renderer } from './types';
import './theme.css';

function parseConfig(query: URLSearchParams): Config {
  const renderer = query.get('renderer') ?? 'webgl';
  const feed = query.get('feed') ?? 'raw';
  if (renderer !== 'svg' && renderer !== 'webgl') throw new Error(`unknown renderer ${renderer}`);
  if (feed !== 'raw' && feed !== 'json' && feed !== 'off') throw new Error(`unknown feed ${feed}`);
  const measure = Number(query.get('measure') ?? '3');
  if (!(measure > 0)) throw new Error(`bad measure ${query.get('measure')}`);
  return {
    bench: query.get('bench') === '1',
    renderer: renderer as Renderer,
    feed: feed as FeedMode,
    scenarios: (query.get('scenarios') ?? '').split(',').filter(Boolean),
    measure,
    shots: query.get('shots') === '1',
  };
}

window.addEventListener('error', (e) => reportError(e.error ?? e.message));
window.addEventListener('unhandledrejection', (e) => reportError(e.reason));

async function main() {
  const config = parseConfig(new URLSearchParams(location.search));
  const response = await fetch('graph.json');
  if (!response.ok) throw new Error(`graph.json: HTTP ${response.status}`);
  const graph = (await response.json()) as Graph;
  createRoot(document.getElementById('root')!).render(<App config={config} graph={graph} />);
}

main().catch(reportError);

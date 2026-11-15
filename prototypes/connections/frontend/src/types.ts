// The graph that the Rust side generates (src/graph.rs), and the page options.

export interface Pin {
  /** The pin center, from the top of the node. */
  y: number;
  width: number;
}

export interface GraphNode {
  x: number;
  y: number;
  w: number;
  h: number;
  title: string;
  family: number;
  inputs: Pin[];
  outputs: Pin[];
}

export interface GraphEdge {
  source: number;
  sourcePin: number;
  target: number;
  targetPin: number;
  width: number;
  /** The index of the first lane (the most significant bit) in the lane states. */
  lane0: number;
  route: [number, number][];
}

export interface Graph {
  nodes: GraphNode[];
  edges: GraphEdge[];
  laneCount: number;
  lanePitch: number;
}

export type Renderer = 'svg' | 'webgl';
export type FeedMode = 'raw' | 'json' | 'off';

export interface Config {
  bench: boolean;
  renderer: Renderer;
  feed: FeedMode;
  scenarios: string[];
  measure: number;
  /** The bench saves a screenshot before it measures each scenario. */
  shots: boolean;
}

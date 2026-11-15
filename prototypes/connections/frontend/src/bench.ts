// The bench driver: it moves the view or the selection in each frame, and it measures the frames.
// The Rust side (src/bench.rs) adds the CPU and GPU use and writes the report.

import { invoke } from '@tauri-apps/api/core';
import type { Lanes } from './lanes';
import type { Config } from './types';
import type { Wires } from './wires';

const MAX_FRAMES = 8192;
const WARMUP_MS = 500;
const CALIBRATE_FRAMES = 60;

/** What the bench needs from the graph view. */
export interface BenchHost {
  setView(x: number, y: number, zoom: number): void;
  fit(): void;
  zoom(): number;
  select(edge: number): void;
  moveRoute(edge: number, k: number, x: number, y: number): void;
  gpu(): string;
}

interface Scenario {
  setup(): void;
  /** `t` is the time since the setup in ms. */
  step(t: number, frame: number): void;
}

const round2 = (v: number) => Math.round(v * 100) / 100;

function percentile(sorted: ArrayLike<number>, p: number): number {
  return sorted.length ? round2(sorted[Math.round((sorted.length - 1) * p)]) : 0;
}

export class Bench {
  private readonly intervals = new Float32Array(MAX_FRAMES);
  private readonly works = new Float32Array(MAX_FRAMES);
  private readonly calibration = new Float32Array(CALIBRATE_FRAMES);
  private count = 0;
  private refresh = 0;
  private index = -1;
  private scenario: Scenario | null = null;
  private phase: 'calibrate' | 'warmup' | 'measure' | 'wait' = 'calibrate';
  private pending = false;
  private measureNext = false;
  private start = 0;
  private measureStart = 0;
  private frame = 0;
  private readonly cx: number;
  private readonly cy: number;

  constructor(
    private readonly config: Config,
    private readonly wires: Wires,
    private readonly lanes: Lanes,
    private readonly host: BenchHost,
    private readonly onError: (e: unknown) => void,
  ) {
    let minX = Infinity;
    let minY = Infinity;
    let maxX = -Infinity;
    let maxY = -Infinity;
    for (const n of wires.graph.nodes) {
      minX = Math.min(minX, n.x);
      minY = Math.min(minY, n.y);
      maxX = Math.max(maxX, n.x + n.w);
      maxY = Math.max(maxY, n.y + n.h);
    }
    this.cx = (minX + maxX) / 2;
    this.cy = (minY + maxY) / 2;
  }

  /** Called at the start of each frame. */
  before(ts: number) {
    if (this.pending) {
      this.pending = false;
      this.index++;
      this.scenario = this.make(this.config.scenarios[this.index]);
      this.scenario.setup();
      this.phase = 'warmup';
      this.start = ts;
      this.frame = 0;
    }
    if (this.measureNext) {
      this.measureNext = false;
      this.phase = 'measure';
      this.measureStart = ts;
      this.count = 0;
      this.lanes.resetStats();
    }
    if (this.phase === 'calibrate' || this.phase === 'wait') return;
    if (this.phase === 'warmup' && ts - this.start >= WARMUP_MS) {
      this.phase = 'wait';
      this.begin().catch(this.onError);
      return;
    }
    if (this.phase === 'measure' && ts - this.measureStart >= this.config.measure * 1000) {
      this.phase = 'wait';
      this.end().catch(this.onError);
      return;
    }
    this.scenario!.step(ts - this.start, this.frame++);
  }

  /** Records a frame: the time since the last frame, and the main-thread time of the frame. */
  record(interval: number, work: number) {
    if (interval <= 0) return;
    if (this.phase === 'calibrate') {
      this.calibration[this.count++] = interval;
      if (this.count === CALIBRATE_FRAMES) {
        this.refresh = this.calibration.slice().sort()[CALIBRATE_FRAMES >> 1];
        this.phase = 'wait';
        this.pending = true;
      }
      return;
    }
    if (this.phase !== 'measure' || this.count >= MAX_FRAMES) return;
    this.intervals[this.count] = interval;
    this.works[this.count] = work;
    this.count++;
  }

  private async begin() {
    if (this.config.shots) {
      const { renderer, feed, scenarios } = this.config;
      const dpr = Math.round(window.devicePixelRatio * 100);
      await invoke('capture', { name: `${renderer}-${feed}-dpr${dpr}-${scenarios[this.index]}` });
    }
    await invoke('scenario_begin');
    this.measureNext = true;
  }

  private async end() {
    await invoke('scenario_end', { stats: this.stats() });
    if (this.index + 1 < this.config.scenarios.length) {
      this.pending = true;
      return;
    }
    const next = await invoke<string | null>('run_done', {
      info: {
        dpr: window.devicePixelRatio,
        gpu: this.host.gpu(),
        refresh_ms: round2(this.refresh),
        ua: navigator.userAgent,
        viewport: [window.innerWidth, window.innerHeight],
        nodes: this.wires.graph.nodes.length,
        edges: this.wires.graph.edges.length,
        lanes: this.wires.graph.laneCount,
      },
    });
    if (next) location.replace(next);
  }

  private stats() {
    const n = this.count;
    const intervals = this.intervals.slice(0, n).sort();
    const works = this.works.slice(0, n).sort();
    const latency = this.lanes.latency.slice(0, this.lanes.latencyCount).sort();
    let total = 0;
    let dropped = 0;
    for (let k = 0; k < n; k++) {
      total += this.intervals[k];
      dropped += Math.max(0, Math.round(this.intervals[k] / this.refresh) - 1);
    }
    return {
      name: this.config.scenarios[this.index],
      fps: total > 0 ? round2((n * 1000) / total) : 0,
      interval_p50: percentile(intervals, 0.5),
      interval_p95: percentile(intervals, 0.95),
      interval_p99: percentile(intervals, 0.99),
      interval_max: n ? round2(intervals[n - 1]) : 0,
      dropped,
      work_p50: percentile(works, 0.5),
      work_p95: percentile(works, 0.95),
      feed_frames: this.lanes.received,
      feed_gaps: this.lanes.gaps,
      latency_p50: percentile(latency, 0.5),
      latency_p99: percentile(latency, 0.99),
      refresh_ms: round2(this.refresh),
      zoom: round2(this.host.zoom()),
      dpr: window.devicePixelRatio,
      dom_nodes: document.getElementsByTagName('*').length,
    };
  }

  /** Puts the graph point (x, y) at the center of the window. */
  private view(x: number, y: number, zoom: number) {
    this.host.setView(window.innerWidth / 2 - x * zoom, window.innerHeight / 2 - y * zoom, zoom);
  }

  private make(name: string): Scenario {
    const { cx, cy, host, wires } = this;
    const { edges } = wires.graph;
    const still = (zoom: number): Scenario => ({ setup: () => this.view(cx, cy, zoom), step: () => {} });
    switch (name) {
      case 'idle-far':
        return { setup: () => host.fit(), step: () => {} };
      case 'idle-near':
        return still(1);
      case 'pan':
        return {
          setup: () => this.view(cx + 1500, cy, 0.8),
          step: (t) => {
            const a = (2 * Math.PI * t) / 6000;
            this.view(cx + 1500 * Math.cos(a), cy + 1500 * Math.sin(a), 0.8);
          },
        };
      case 'zoom':
        return {
          setup: () => this.view(cx, cy, 0.2),
          step: (t) => {
            const s = 0.5 - 0.5 * Math.cos((2 * Math.PI * t) / 6000);
            this.view(cx, cy, Math.exp(Math.log(0.2) + (Math.log(2) - Math.log(0.2)) * s));
          },
        };
      case 'select': {
        const halfW = window.innerWidth / 2 - 40;
        const halfH = window.innerHeight / 2 - 40;
        const near: number[] = [];
        for (let i = 0; i < edges.length; i++) {
          const dx = Math.abs(wires.mid[i * 2] - cx);
          const dy = Math.abs(wires.mid[i * 2 + 1] - cy);
          if (edges[i].width > 1 && dx < halfW && dy < halfH) near.push(i);
        }
        if (near.length === 0) throw new Error('select: no bus is near the center');
        return {
          setup: () => this.view(cx, cy, 1),
          step: (_t, frame) => {
            if (frame % 6 === 0) host.select(near[(frame / 6) % near.length]);
          },
        };
      }
      case 'drag': {
        let best = -1;
        let bestD = Infinity;
        for (let i = 0; i < edges.length; i++) {
          if (edges[i].route.length < 2) continue;
          const [x, y] = edges[i].route[1];
          const d = Math.hypot(x - cx, y - cy);
          if (d < bestD) {
            bestD = d;
            best = i;
          }
        }
        if (best < 0) throw new Error('drag: the graph has no routed edge');
        const [bx, by] = edges[best].route[1];
        return {
          setup: () => {
            this.view(bx, by, 1);
            host.select(best);
          },
          step: (t) => {
            const a = (2 * Math.PI * t) / 2000;
            host.moveRoute(best, 1, bx + 80 * Math.cos(a), by + 80 * Math.sin(a));
          },
        };
      }
      default:
        throw new Error(`unknown scenario ${name}`);
    }
  }
}

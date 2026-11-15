// The lane states from the Rust feed (src/feed.rs): [u32 frame id][f64 Unix ms][packed bits].

import { Channel, invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { FeedMode } from './types';

const HEADER_BYTES = 12;
const MAX_SAMPLES = 8192;

export class Lanes {
  /** Lane k is (bits[k >> 3] >> (k & 7)) & 1. */
  readonly bits: Uint8Array;
  /** Goes up by one for each frame that arrives. */
  version = 0;
  received = 0;
  gaps = 0;
  readonly latency = new Float64Array(MAX_SAMPLES);
  latencyCount = 0;
  private lastId = -1;

  constructor(laneCount: number) {
    this.bits = new Uint8Array(Math.ceil(laneCount / 8));
  }

  bit(lane: number): number {
    return (this.bits[lane >> 3] >> (lane & 7)) & 1;
  }

  apply(frame: Uint8Array) {
    if (frame.length !== HEADER_BYTES + this.bits.length) {
      throw new Error(`a feed frame has ${frame.length} bytes, not ${HEADER_BYTES + this.bits.length}`);
    }
    const view = new DataView(frame.buffer, frame.byteOffset, HEADER_BYTES);
    const id = view.getUint32(0, true);
    const sentMs = view.getFloat64(4, true);
    if (this.lastId >= 0 && id !== this.lastId + 1) this.gaps++;
    this.lastId = id;
    this.received++;
    if (this.latencyCount < MAX_SAMPLES) {
      this.latency[this.latencyCount++] = performance.timeOrigin + performance.now() - sentMs;
    }
    this.bits.set(frame.subarray(HEADER_BYTES));
    this.version++;
  }

  resetStats() {
    this.received = 0;
    this.gaps = 0;
    this.latencyCount = 0;
  }
}

function toBytes(message: unknown): Uint8Array {
  if (message instanceof ArrayBuffer) return new Uint8Array(message);
  if (message instanceof Uint8Array) return message;
  if (Array.isArray(message)) return Uint8Array.from(message as number[]);
  throw new Error('the feed sent a frame of an unknown type');
}

/** Starts the Rust feed. "raw" sends bytes by a channel, "json" sends a number array by an event. */
export async function startFeed(mode: FeedMode, lanes: Lanes, onError: (e: unknown) => void) {
  if (mode === 'off') return;
  const guard = (message: unknown) => {
    try {
      lanes.apply(toBytes(message));
    } catch (e) {
      onError(e);
    }
  };
  const channel = new Channel<unknown>();
  if (mode === 'json') {
    await listen<number[]>('lanes', (event) => guard(event.payload));
  } else {
    channel.onmessage = guard;
  }
  await invoke('start_feed', { mode, channel });
}

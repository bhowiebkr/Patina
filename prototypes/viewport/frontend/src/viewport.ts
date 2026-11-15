// One viewport panel. It sends its rectangle and its input to the render thread. On the copy
// paths it also draws the frames that come back: through a Tauri channel, or from WebView2
// shared memory that it polls in each animation frame.

import { Channel, invoke } from '@tauri-apps/api/core';

/** The page counters of the bench. The bench clears them at the start of each scenario. */
export const page = {
  since: performance.now(),
  ticks: 0,
  draws: 0,
  uploadMs: [] as number[],
  latencyMs: [] as number[],
  inputs: 0,
  reset() {
    this.since = performance.now();
    this.ticks = this.draws = this.inputs = 0;
    this.uploadMs = [];
    this.latencyMs = [];
  },
};

export function percentile(values: number[], p: number): number {
  if (values.length === 0) return 0;
  const sorted = [...values].sort((a, b) => a - b);
  return Math.round(sorted[Math.round((sorted.length - 1) * p)] * 100) / 100;
}

/** The difference between the Rust wall clock and the page clock, in ms. */
let clockOffset = 0;

export async function syncClock(): Promise<number> {
  let best = Infinity;
  for (let i = 0; i < 5; i++) {
    const t0 = performance.timeOrigin + performance.now();
    const rust = await invoke<number>('now');
    const t1 = performance.timeOrigin + performance.now();
    if (t1 - t0 < best) {
      best = t1 - t0;
      clockOffset = rust - (t0 + t1) / 2;
    }
  }
  return clockOffset;
}

const pageNow = () => performance.timeOrigin + performance.now();

interface SharedBuffer {
  key: number;
  buffer: ArrayBuffer;
  states: Uint32Array[];
  pixels: Uint8Array[];
}

interface WebView2 {
  addEventListener(type: 'sharedbufferreceived', listener: (e: SharedBufferEvent) => void): void;
  releaseBuffer(buffer: ArrayBuffer): void;
}

interface SharedBufferEvent {
  getBuffer(): ArrayBuffer;
  additionalData: { vp: number; key: number; slots: number; slot_len: number };
}

const webview = (window as unknown as { chrome?: { webview?: WebView2 } }).chrome?.webview;
const shared = new Map<number, SharedBuffer>();

webview?.addEventListener('sharedbufferreceived', (e) => {
  const { vp, key, slots, slot_len: len } = e.additionalData;
  const buffer = e.getBuffer();
  const old = shared.get(vp);
  if (old && old.key > key) {
    webview.releaseBuffer(buffer);
    return;
  }
  if (old) webview.releaseBuffer(old.buffer);
  const states = [];
  const pixels = [];
  for (let i = 0; i < slots; i++) {
    states.push(new Uint32Array(buffer, i * len, 8));
    pixels.push(new Uint8Array(buffer, i * len + 32, len - 32));
  }
  shared.set(vp, { key, buffer, states, pixels });
});

const VERTEX = `#version 300 es
out vec2 uv;
void main() {
  vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2));
  uv = vec2(p.x, 1.0 - p.y);
  gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}`;

const FRAGMENT = `#version 300 es
precision mediump float;
uniform sampler2D image;
in vec2 uv;
out vec4 color;
void main() {
  color = vec4(texture(image, uv).rgb, 1.0);
}`;

/** Draws the frames of a copy viewport with WebGL2. The rows come top-down from the GPU. */
class Drawer {
  private gl: WebGL2RenderingContext;
  private texture: WebGLTexture | null = null;
  private width = 0;
  private height = 0;

  constructor(canvas: HTMLCanvasElement) {
    const gl = canvas.getContext('webgl2', { alpha: false, antialias: false, depth: false });
    if (!gl) throw new Error('no WebGL2');
    this.gl = gl;
    const program = gl.createProgram();
    for (const [kind, source] of [
      [gl.VERTEX_SHADER, VERTEX],
      [gl.FRAGMENT_SHADER, FRAGMENT],
    ] as const) {
      const shader = gl.createShader(kind)!;
      gl.shaderSource(shader, source);
      gl.compileShader(shader);
      gl.attachShader(program, shader);
    }
    gl.linkProgram(program);
    if (!gl.getProgramParameter(program, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(program) ?? 'link');
    gl.useProgram(program);
    gl.bindVertexArray(gl.createVertexArray());
  }

  draw(pixels: Uint8Array, width: number, height: number, stride: number) {
    const gl = this.gl;
    if (width !== this.width || height !== this.height) {
      gl.deleteTexture(this.texture);
      this.texture = gl.createTexture();
      gl.bindTexture(gl.TEXTURE_2D, this.texture);
      gl.texStorage2D(gl.TEXTURE_2D, 1, gl.RGBA8, width, height);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
      [this.width, this.height] = [width, height];
    }
    gl.pixelStorei(gl.UNPACK_ROW_LENGTH, stride / 4);
    gl.texSubImage2D(gl.TEXTURE_2D, 0, 0, 0, width, height, gl.RGBA, gl.UNSIGNED_BYTE, pixels);
    gl.viewport(0, 0, gl.drawingBufferWidth, gl.drawingBufferHeight);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
  }
}

export class Viewport {
  readonly el: HTMLDivElement;
  registered = false;
  /** The last rectangle sent to Rust: physical x, y, width, height, or null when hidden. */
  rect: number[] | null = null;
  private canvas: HTMLCanvasElement | null = null;
  private drawer: Drawer | null = null;
  private channelFrame: ArrayBuffer | null = null;
  /** The page times of the inputs that no drawn frame shows yet. */
  private pending: number[] = [];
  private pendingFirst = 1;
  private lastX = 0;
  private lastY = 0;

  constructor(
    readonly id: number,
    readonly mode: string,
  ) {
    this.el = document.createElement('div');
    this.el.className = 'vp';
    this.el.tabIndex = 0;
    if (mode !== 'native') {
      this.canvas = document.createElement('canvas');
      this.el.append(this.canvas);
      this.drawer = new Drawer(this.canvas);
    }
    this.el.addEventListener('pointerdown', (e) => {
      this.el.focus();
      this.el.setPointerCapture(e.pointerId);
      [this.lastX, this.lastY] = [e.clientX, e.clientY];
    });
    this.el.addEventListener('pointermove', (e) => {
      if ((e.buttons & 1) === 0) return;
      const dpr = devicePixelRatio;
      this.send(e, (e.clientX - this.lastX) * dpr, (e.clientY - this.lastY) * dpr, 0, false);
      [this.lastX, this.lastY] = [e.clientX, e.clientY];
    });
    this.el.addEventListener(
      'wheel',
      (e) => {
        e.preventDefault();
        this.send(e, 0, 0, e.deltaY, false);
      },
      { passive: false },
    );
    this.el.addEventListener('keydown', (e) => {
      if (e.key === 'Tab') return;
      e.preventDefault();
      this.send(e, 0, 0, 0, true);
    });
  }

  private send(e: Event, dx: number, dy: number, wheel: number, key: boolean) {
    if (!this.registered) return;
    const t = performance.timeOrigin + e.timeStamp;
    this.pending.push(t);
    page.inputs++;
    invoke('input', { id: this.id, dx, dy, wheel, key, t: t + clockOffset }).catch(() => {});
  }

  async register() {
    const channel = new Channel<ArrayBuffer>();
    channel.onmessage = (data) => {
      this.channelFrame = data instanceof ArrayBuffer ? data : new Uint8Array(data as ArrayLike<number>).buffer;
      invoke('ack', { id: this.id }).catch(() => {});
    };
    this.pending = [];
    this.pendingFirst = 1;
    this.rect = null;
    this.channelFrame = null;
    await invoke('register', { id: this.id, mode: this.mode, channel });
    this.registered = true;
  }

  async unregister() {
    this.registered = false;
    await invoke('unregister', { id: this.id });
  }

  /** The rectangle of the panel in physical pixels of the client area, or null when hidden. */
  physRect(): number[] | null {
    if (!this.el.isConnected || this.el.offsetParent === null) return null;
    const r = this.el.getBoundingClientRect();
    const dpr = devicePixelRatio;
    const [x0, y0, x1, y1] = [r.left, r.top, r.right, r.bottom].map((v) => Math.round(v * dpr));
    return x1 > x0 && y1 > y0 ? [x0, y0, x1 - x0, y1 - y0] : null;
  }

  /** Runs in each animation frame: sends a changed rectangle and draws the newest frame. */
  tick() {
    if (!this.registered) return;
    const rect = this.physRect();
    if (String(rect) !== String(this.rect)) {
      this.rect = rect;
      if (rect && this.canvas) [this.canvas.width, this.canvas.height] = [rect[2], rect[3]];
      invoke('rect', { id: this.id, rect }).catch(() => {});
    }
    if (this.drawer && rect) this.drawLatest();
  }

  private drawLatest() {
    if (this.mode === 'channel') {
      const frame = this.channelFrame;
      if (!frame) return;
      this.channelFrame = null;
      const [, seq, w, h, stride] = new Uint32Array(frame, 0, 6);
      this.drawFrame(new Uint8Array(frame, 24), seq, w, h, stride);
      return;
    }
    const buffer = shared.get(this.id);
    if (!buffer) return;
    let newest = -1;
    for (let i = 0; i < buffer.states.length; i++) {
      const s = buffer.states[i];
      if (s[0] === 1 && (newest < 0 || s[1] > buffer.states[newest][1])) newest = i;
    }
    if (newest < 0) return;
    const [, , seq, w, h, stride] = buffer.states[newest];
    this.drawFrame(buffer.pixels[newest], seq, w, h, stride);
    // The slots go back to Rust after the upload: texSubImage2D copies the pixels.
    for (const s of buffer.states) if (s[0] === 1) s[0] = 0;
  }

  private drawFrame(pixels: Uint8Array, seq: number, w: number, h: number, stride: number) {
    const begin = performance.now();
    this.drawer!.draw(pixels, w, h, stride);
    const end = pageNow();
    page.uploadMs.push(performance.now() - begin);
    page.draws++;
    while (this.pending.length > 0 && this.pendingFirst <= seq) {
      page.latencyMs.push(end - this.pending.shift()!);
      this.pendingFirst++;
    }
  }
}

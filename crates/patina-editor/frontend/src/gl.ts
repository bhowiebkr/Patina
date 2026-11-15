// The WebGL2 renderer of the connections (spec 15.1): one instanced draw of line segments on a
// canvas under the nodes. A texture row holds the points and the miter normals of each wire.

import { MAX_POINTS, STRIDE, type WireStore } from './wires';

/** The line widths, in graph pixels, and the colors of docs/gui-visual-guide.md. */
const WIRE_WIDTH = 2;
const SELECT_WIDTH = 8;
const WIRE = [187 / 255, 187 / 255, 181 / 255];
const ACCENT = [223 / 255, 166 / 255, 75 / 255];
/** Above this number of changed wires, one upload of the whole texture is faster. */
const ROW_UPLOADS = 64;

const VERTEX = `#version 300 es
precision highp float;
layout(location = 0) in uvec2 inst; // wire slot, segment
uniform highp sampler2D points;
uniform vec3 view;
uniform vec2 size;
uniform float width;
out float vAcross;
out float vHalf;
void main() {
  // The two ends of a segment use the miter normals, so the next segment starts at the same edge.
  int v = gl_VertexID;
  float along = (v == 1 || v == 4 || v == 5) ? 1.0 : 0.0;
  float side = (v == 2 || v == 3 || v == 5) ? 1.0 : -1.0;
  vec4 p = texelFetch(points, ivec2(int(inst.y) + int(along), int(inst.x)), 0);
  float halfW = max(width * view.z, 1.0) * 0.5;
  float ext = halfW + 1.0;
  vec2 pos = p.xy * view.z + view.xy + p.zw * side * ext;
  vec2 clip = pos / size * 2.0 - 1.0;
  gl_Position = vec4(clip.x, -clip.y, 0.0, 1.0);
  vAcross = side * ext;
  vHalf = halfW;
}`;

const FRAGMENT = `#version 300 es
precision mediump float;
uniform vec3 color;
in float vAcross;
in float vHalf;
out vec4 outColor;
void main() {
  float a = clamp(vHalf + 0.5 - abs(vAcross), 0.0, 1.0);
  outColor = vec4(color * a, a);
}`;

function compile(gl: WebGL2RenderingContext, type: number, source: string): WebGLShader {
  const shader = gl.createShader(type)!;
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) throw new Error(`shader: ${gl.getShaderInfoLog(shader)}`);
  return shader;
}

class Instances {
  readonly vao: WebGLVertexArrayObject;
  readonly buffer: WebGLBuffer;
  count = 0;

  constructor(private readonly gl: WebGL2RenderingContext) {
    this.vao = gl.createVertexArray()!;
    this.buffer = gl.createBuffer()!;
    gl.bindVertexArray(this.vao);
    gl.bindBuffer(gl.ARRAY_BUFFER, this.buffer);
    gl.enableVertexAttribArray(0);
    gl.vertexAttribIPointer(0, 2, gl.UNSIGNED_SHORT, 4, 0);
    gl.vertexAttribDivisor(0, 1);
    gl.bindVertexArray(null);
  }

  upload(data: Uint16Array) {
    const gl = this.gl;
    gl.bindBuffer(gl.ARRAY_BUFFER, this.buffer);
    gl.bufferData(gl.ARRAY_BUFFER, data, gl.DYNAMIC_DRAW);
    this.count = data.length / 2;
  }

  dispose() {
    this.gl.deleteVertexArray(this.vao);
    this.gl.deleteBuffer(this.buffer);
  }
}

/** The segments of the wires, or of the selected wires only. */
function segments(store: WireStore, selectedOnly: boolean): Uint16Array {
  let n = 0;
  for (let s = 0; s < store.counts.length; s++) if (!selectedOnly || store.selected[s]) n += Math.max(0, store.counts[s] - 1);
  const out = new Uint16Array(n * 2);
  let at = 0;
  for (let s = 0; s < store.counts.length; s++) {
    if (selectedOnly && !store.selected[s]) continue;
    for (let k = 0; k < store.counts[s] - 1; k++, at += 2) {
      out[at] = s;
      out[at + 1] = k;
    }
  }
  return out;
}

export class GlWires {
  private readonly gl: WebGL2RenderingContext;
  private program!: WebGLProgram;
  private uniforms!: Record<'points' | 'view' | 'size' | 'width' | 'color', WebGLUniformLocation | null>;
  private texture!: WebGLTexture;
  private all!: Instances;
  private chosen!: Instances;
  private maxRows = 0;
  private width = 1;
  private height = 1;
  private sized = false;
  private last = [NaN, NaN, NaN];

  constructor(
    canvas: HTMLCanvasElement,
    private readonly store: WireStore,
  ) {
    const gl = canvas.getContext('webgl2', { antialias: false, premultipliedAlpha: true });
    if (!gl) throw new Error('WebGL2 is not available');
    this.gl = gl;
    this.init();
  }

  /** Makes the GPU resources, also after a lost context comes back. */
  init() {
    const gl = this.gl;
    const program = gl.createProgram()!;
    gl.attachShader(program, compile(gl, gl.VERTEX_SHADER, VERTEX));
    gl.attachShader(program, compile(gl, gl.FRAGMENT_SHADER, FRAGMENT));
    gl.linkProgram(program);
    if (!gl.getProgramParameter(program, gl.LINK_STATUS)) throw new Error(`program: ${gl.getProgramInfoLog(program)}`);
    this.program = program;
    const u = (name: string) => gl.getUniformLocation(program, name);
    this.uniforms = { points: u('points'), view: u('view'), size: u('size'), width: u('width'), color: u('color') };
    this.maxRows = gl.getParameter(gl.MAX_TEXTURE_SIZE) as number;
    this.texture = gl.createTexture()!;
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, this.texture);
    for (const p of [gl.TEXTURE_MIN_FILTER, gl.TEXTURE_MAG_FILTER]) gl.texParameteri(gl.TEXTURE_2D, p, gl.NEAREST);
    for (const p of [gl.TEXTURE_WRAP_S, gl.TEXTURE_WRAP_T]) gl.texParameteri(gl.TEXTURE_2D, p, gl.CLAMP_TO_EDGE);
    this.all = new Instances(gl);
    this.chosen = new Instances(gl);
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
    this.store.grown = true;
    this.store.layout = true;
    this.last[2] = NaN;
  }

  dispose() {
    const gl = this.gl;
    gl.deleteProgram(this.program);
    gl.deleteTexture(this.texture);
    this.all.dispose();
    this.chosen.dispose();
  }

  /** Sets the canvas size in CSS pixels and in device pixels. */
  resize(width: number, height: number, deviceWidth: number, deviceHeight: number) {
    const canvas = this.gl.canvas as HTMLCanvasElement;
    this.width = Math.max(1, width);
    this.height = Math.max(1, height);
    canvas.width = Math.max(1, deviceWidth);
    canvas.height = Math.max(1, deviceHeight);
    this.sized = true;
    this.last[2] = NaN;
  }

  /** Draws when the view or a wire changed. Returns false if the GPU cannot hold the wires. */
  draw(tx: number, ty: number, zoom: number): boolean {
    const { gl, store } = this;
    const changed = store.grown || store.layout || store.changed.size > 0;
    if (!this.sized || (!changed && tx === this.last[0] && ty === this.last[1] && zoom === this.last[2])) return true;
    const rows = store.counts.length;
    // shortcut: one texture row for each wire, so above MAX_TEXTURE_SIZE wires SVG draws them; pack the rows for larger graphs.
    if (rows > this.maxRows) return false;
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, this.texture);
    if (store.grown || store.changed.size > ROW_UPLOADS) {
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA32F, MAX_POINTS, rows, 0, gl.RGBA, gl.FLOAT, store.points);
    } else {
      for (const s of store.changed) {
        const row = store.points.subarray(s * MAX_POINTS * STRIDE, (s + 1) * MAX_POINTS * STRIDE);
        gl.texSubImage2D(gl.TEXTURE_2D, 0, 0, s, MAX_POINTS, 1, gl.RGBA, gl.FLOAT, row);
      }
    }
    if (store.layout) {
      this.all.upload(segments(store, false));
      this.chosen.upload(segments(store, true));
    }
    store.grown = false;
    store.layout = false;
    store.changed.clear();

    gl.viewport(0, 0, gl.drawingBufferWidth, gl.drawingBufferHeight);
    gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT);
    gl.useProgram(this.program);
    gl.uniform1i(this.uniforms.points, 0);
    gl.uniform3f(this.uniforms.view, tx, ty, zoom);
    gl.uniform2f(this.uniforms.size, this.width, this.height);
    const pass = (b: Instances, width: number, color: number[]) => {
      if (b.count === 0) return;
      gl.uniform1f(this.uniforms.width, width);
      gl.uniform3f(this.uniforms.color, color[0], color[1], color[2]);
      gl.bindVertexArray(b.vao);
      gl.drawArraysInstanced(gl.TRIANGLES, 0, 6, b.count);
    };
    pass(this.chosen, SELECT_WIDTH, ACCENT);
    pass(this.all, WIRE_WIDTH, WIRE);
    gl.bindVertexArray(null);
    this.last[0] = tx;
    this.last[1] = ty;
    this.last[2] = zoom;
    return true;
  }
}

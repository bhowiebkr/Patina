// The WebGL2 renderer of the connections (spec 15.1): one instanced draw of line segments on a
// canvas under the nodes. A texture row holds the points and the miter normals of each wire.

import { HAS_LANES, instances, LIT, MAX_POINTS, STALE, STRIDE, type WireStore } from './wires';

/** The line widths and the lane pitch, in graph pixels, and the colors of docs/gui-visual-guide.md. */
const WIRE_WIDTH = 2;
const SELECT_WIDTH = 8;
const LANE_WIDTH = 2;
/** The distance between the centers of two lanes. The dark gap between them is 1 px. */
export const PITCH = 3;
/** Below this zoom, a bus draws as one line of FAR_WIDTH (15.9). */
export const FAR_ZOOM = 0.5;
const FAR_WIDTH = 4;
const WIRE = [187, 187, 181];
const ACCENT = [223, 166, 75];
/** A lane with the bit 1, and with the bit 0. An off lane stays visible (15.9). */
const LANE_ON = [238, 238, 235];
const LANE_OFF = [85, 85, 81];
/** Above this number of changed wires, one upload of the whole texture is faster. */
const ROW_UPLOADS = 64;

const vec3 = (c: number[]) => `vec3(${c.map((v) => (v / 255).toFixed(4)).join(', ')})`;
const float = (v: number) => v.toFixed(2);

const VERTEX = `#version 300 es
precision highp float;
layout(location = 0) in uvec4 inst; // wire slot, segment, lane, flags
uniform highp sampler2D points;
uniform vec3 view;
uniform vec2 size;
uniform bool chosen;
out float vAcross;
out float vHalf;
out vec3 vColor;
void main() {
  uint count = (inst.w & 63u) + 1u;
  bool lanes = (inst.w & ${HAS_LANES}u) != 0u;
  bool far = view.z < ${float(FAR_ZOOM)} && count > 1u;
  // The distance between the outer lanes of a bus.
  float bus = far ? 0.0 : float(count - 1u) * ${float(PITCH)};
  float width = ${float(LANE_WIDTH)};
  float offset = 0.0;
  if (chosen) {
    width = ${float(SELECT_WIDTH)} + bus;
    vColor = ${vec3(ACCENT)};
  } else if (!lanes) {
    width = ${float(WIRE_WIDTH)};
    vColor = ${vec3(WIRE)};
  } else if (far) {
    width = ${float(FAR_WIDTH)};
    vColor = ${vec3(WIRE)};
    // One line: the other lanes have no area.
    if (inst.z > 0u) {
      gl_Position = vec4(2.0, 2.0, 2.0, 1.0);
      return;
    }
  } else {
    // Lane 0 is the most significant bit, on the left side of the direction of the wire.
    offset = float(inst.z) * ${float(PITCH)} - bus * 0.5;
    bool lit = (inst.w & ${LIT}u) != 0u;
    vec3 on = (inst.w & ${STALE}u) != 0u ? mix(${vec3(LANE_OFF)}, ${vec3(LANE_ON)}, 0.5) : ${vec3(LANE_ON)};
    vColor = lit ? on : ${vec3(LANE_OFF)};
  }
  // The two ends of a segment use the miter normals, so the next segment starts at the same edge.
  int v = gl_VertexID;
  float along = (v == 1 || v == 4 || v == 5) ? 1.0 : 0.0;
  float side = (v == 2 || v == 3 || v == 5) ? 1.0 : -1.0;
  vec4 p = texelFetch(points, ivec2(int(inst.y) + int(along), int(inst.x)), 0);
  float halfW = max(width * view.z, 1.0) * 0.5;
  float ext = halfW + 1.0;
  vec2 pos = p.xy * view.z + view.xy + p.zw * (offset * view.z + side * ext);
  vec2 clip = pos / size * 2.0 - 1.0;
  gl_Position = vec4(clip.x, -clip.y, 0.0, 1.0);
  vAcross = side * ext;
  vHalf = halfW;
}`;

const FRAGMENT = `#version 300 es
precision mediump float;
in float vAcross;
in float vHalf;
in vec3 vColor;
out vec4 outColor;
void main() {
  float a = clamp(vHalf + 0.5 - abs(vAcross), 0.0, 1.0);
  outColor = vec4(vColor * a, a);
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
    gl.vertexAttribIPointer(0, 4, gl.UNSIGNED_SHORT, 8, 0);
    gl.vertexAttribDivisor(0, 1);
    gl.bindVertexArray(null);
  }

  upload(data: Uint16Array) {
    const gl = this.gl;
    gl.bindBuffer(gl.ARRAY_BUFFER, this.buffer);
    gl.bufferData(gl.ARRAY_BUFFER, data, gl.DYNAMIC_DRAW);
    this.count = data.length / 4;
  }

  dispose() {
    this.gl.deleteVertexArray(this.vao);
    this.gl.deleteBuffer(this.buffer);
  }
}

export class GlWires {
  private readonly gl: WebGL2RenderingContext;
  private program!: WebGLProgram;
  private uniforms!: Record<'points' | 'view' | 'size' | 'chosen', WebGLUniformLocation | null>;
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
    this.uniforms = { points: u('points'), view: u('view'), size: u('size'), chosen: u('chosen') };
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
      this.all.upload(instances(store, false));
      this.chosen.upload(instances(store, true));
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
    const pass = (b: Instances, chosen: boolean) => {
      if (b.count === 0) return;
      gl.uniform1i(this.uniforms.chosen, chosen ? 1 : 0);
      gl.bindVertexArray(b.vao);
      gl.drawArraysInstanced(gl.TRIANGLES, 0, 6, b.count);
    };
    pass(this.chosen, true);
    pass(this.all, false);
    gl.bindVertexArray(null);
    this.last[0] = tx;
    this.last[1] = ty;
    this.last[2] = zoom;
    return true;
  }
}

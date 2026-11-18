// The WebGL2 renderer of the connections (spec 15.1): one instanced draw of line segments on a
// canvas under the nodes. A texture row holds the points and the miter normals of each wire.

import { LANE_COLORS, PITCH } from '../canvas/bus';
import { DASHED, FAMILY_SHIFT, HAS_LANES, HOVER, instances, LIT, MAX_POINTS, STALE, STRIDE, type WireStore } from './wires';

/** The line widths and the lane pitch, in graph pixels, and the colors of docs/gui-visual-guide.md. */
const WIRE_WIDTH = 2;
const SELECT_WIDTH = 8;
const LANE_WIDTH = 3;
/** The dark edge on each side of a lane. Two edges fill the gap between two lanes. */
const EDGE = (PITCH - LANE_WIDTH) / 2;
/** The soft shadow on each side of a ribbon, and its darkest alpha. */
const SHADOW = 4;
const SHADOW_ALPHA = 0.55;
/** Below this zoom, the nodes, their GUIs and the wires take the simple form together (15.1). A bus is one line of FAR_WIDTH (15.9). */
export const FAR_ZOOM = 0.25;
const FAR_WIDTH = 4;
const WIRE = [187, 187, 181];
const ACCENT = [223, 166, 75];
/** A neutral lane with the bit 0. An off lane stays visible (15.9). The lit lanes have LANE_COLORS. */
const LANE_OFF = [85, 85, 81];
const LANE_EDGE = [12, 12, 11];
/** An off lane of a family is a dark tint of its lit color. */
const OFF_TINT = 0.38;
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
// 0: the accent under a selected wire, 1: the shadow under a ribbon, 2: the wires.
uniform int mode;
const vec3 FAMILY[${LANE_COLORS.length}] = vec3[${LANE_COLORS.length}](${LANE_COLORS.map(vec3).join(', ')});
out float vAcross;
out float vHalf;
out float vCore;
out vec3 vColor;
flat out int vKind;
// The distance along the segment and the dash period in screen pixels, 0 for a solid line.
out float vAlong;
flat out float vPeriod;
void main() {
  uint count = (inst.w & 63u) + 1u;
  bool lanes = (inst.w & ${HAS_LANES}u) != 0u;
  bool far = view.z < ${float(FAR_ZOOM)} && count > 1u;
  // The distance between the outer lanes of a bus.
  float bus = far ? 0.0 : float(count - 1u) * ${float(PITCH)};
  bool ribbon = lanes && !far;
  // The full width of a ribbon, with the outer edges.
  float full = bus + ${float(PITCH)};
  float width = ${float(LANE_WIDTH)};
  float offset = 0.0;
  float edge = 0.0;
  uint family = (inst.w >> ${FAMILY_SHIFT}u) & 7u;
  vKind = 0;
  if (mode == 0) {
    width = ribbon ? full + 4.0 : ${float(SELECT_WIDTH)};
    vColor = ${vec3(ACCENT)};
  } else if (mode == 1) {
    // Lane 0 draws the one shadow of the ribbon.
    if (!ribbon || inst.z > 0u) {
      gl_Position = vec4(2.0, 2.0, 2.0, 1.0);
      return;
    }
    width = full;
    edge = ${float(SHADOW)};
    vColor = vec3(0.0);
    vKind = 2;
  } else if (!lanes) {
    width = ${float(WIRE_WIDTH)};
    vColor = ${vec3(WIRE)};
  } else if (far) {
    width = ${float(FAR_WIDTH)};
    vColor = family == 0u ? ${vec3(WIRE)} : FAMILY[family];
    // One line: the other lanes have no area.
    if (inst.z > 0u) {
      gl_Position = vec4(2.0, 2.0, 2.0, 1.0);
      return;
    }
  } else {
    // Lane 0 is the most significant bit, on the left side of the direction of the wire.
    offset = float(inst.z) * ${float(PITCH)} - bus * 0.5;
    bool lit = (inst.w & ${LIT}u) != 0u;
    vec3 off = family == 0u ? ${vec3(LANE_OFF)} : FAMILY[family] * ${float(OFF_TINT)};
    vec3 on = (inst.w & ${STALE}u) != 0u ? mix(off, FAMILY[family], 0.5) : FAMILY[family];
    vColor = lit ? on : off;
    edge = ${float(EDGE)};
    // 3: the lane under the pointer (15.9).
    vKind = (inst.w & ${HOVER}u) != 0u ? 3 : 1;
  }
  // The two ends of a segment use the miter normals, so the next segment starts at the same edge.
  int v = gl_VertexID;
  float along = (v == 1 || v == 4 || v == 5) ? 1.0 : 0.0;
  float side = (v == 2 || v == 3 || v == 5) ? 1.0 : -1.0;
  vec4 p0 = texelFetch(points, ivec2(int(inst.y), int(inst.x)), 0);
  vec4 p1 = texelFetch(points, ivec2(int(inst.y) + 1, int(inst.x)), 0);
  vec4 p = along > 0.5 ? p1 : p0;
  // A cross-domain wire is dashed, and its selection too. The dash starts again at each segment.
  vAlong = along * length(p1.xy - p0.xy) * view.z;
  vPeriod = (inst.w & ${DASHED}u) != 0u && mode != 1 ? max(10.0 * view.z, 5.0) : 0.0;
  float core = max(width * view.z, 1.0) * 0.5;
  float halfW = core + edge * view.z;
  float ext = halfW + 1.0;
  vec2 pos = p.xy * view.z + view.xy + p.zw * (offset * view.z + side * ext);
  vec2 clip = pos / size * 2.0 - 1.0;
  gl_Position = vec4(clip.x, -clip.y, 0.0, 1.0);
  vAcross = side * ext;
  vHalf = halfW;
  vCore = core;
}`;

const FRAGMENT = `#version 300 es
precision mediump float;
in float vAcross;
in float vHalf;
in float vCore;
in vec3 vColor;
flat in int vKind;
in float vAlong;
flat in float vPeriod;
out vec4 outColor;
void main() {
  float d = abs(vAcross);
  float a = clamp(vHalf + 0.5 - d, 0.0, 1.0);
  if (vPeriod > 0.0) {
    // A dash is 60 % of the period, with a smooth end.
    float m = mod(vAlong, vPeriod);
    a *= clamp(min(m, vPeriod * 0.6 - m) + 0.5, 0.0, 1.0);
  }
  vec3 c = vColor;
  if (vKind == 2) {
    // The shadow fades from the edge of the ribbon to its outer side.
    float s = clamp((vHalf - d) / max(vHalf - vCore, 1.0), 0.0, 1.0);
    a = ${float(SHADOW_ALPHA)} * s * s;
  } else if (vKind != 0) {
    // A round lane: darker sides, a narrow highlight at the center and a dark edge.
    float u = clamp(d / vCore, 0.0, 1.0);
    c = vColor * (1.0 - 0.4 * u * u + 0.2 * (1.0 - smoothstep(0.0, 0.45, u)));
    // The lane under the pointer is brighter and has an accent edge.
    if (vKind == 3) c = mix(c, vec3(1.0), 0.3);
    c = mix(c, vKind == 3 ? ${vec3(ACCENT)} : ${vec3(LANE_EDGE)}, clamp(d - vCore + 0.5, 0.0, 1.0));
  }
  outColor = vec4(min(c, 1.0) * a, a);
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
  private uniforms!: Record<'points' | 'view' | 'size' | 'mode', WebGLUniformLocation | null>;
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
    this.uniforms = { points: u('points'), view: u('view'), size: u('size'), mode: u('mode') };
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
    const pass = (b: Instances, mode: number) => {
      if (b.count === 0) return;
      gl.uniform1i(this.uniforms.mode, mode);
      gl.bindVertexArray(b.vao);
      gl.drawArraysInstanced(gl.TRIANGLES, 0, 6, b.count);
    };
    pass(this.all, 1);
    pass(this.chosen, 0);
    pass(this.all, 2);
    gl.bindVertexArray(null);
    this.last[0] = tx;
    this.last[1] = ty;
    this.last[2] = zoom;
    return true;
  }
}

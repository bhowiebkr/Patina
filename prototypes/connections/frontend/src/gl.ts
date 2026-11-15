// The WebGL2 renderer: one instanced draw of line segments on a canvas under the nodes. The lane
// states go to the GPU as the packed bits of the feed. A 2D canvas above it draws the badges.

import type { Lanes } from './lanes';
import { BADGE_BG, BADGE_FONT, BADGE_FONT_SIZE, BADGE_HEIGHT, BADGE_TEXT, SELECT_EXTRA } from './theme';
import { MAX_POINTS, NEAR_ZOOM, type Wires } from './wires';

const KIND_BOOL = 0;
const KIND_LANE = 1;
const KIND_FAR_BUS = 2;
const KIND_SELECT = 3;

const VERTEX = `#version 300 es
precision highp float;
precision highp int;
layout(location = 0) in ivec4 inst; // edge, segment, lane, (arg + 64) * 4 + kind
uniform highp sampler2D points;
uniform highp usampler2D bits;
uniform vec3 view;
uniform vec2 size;
uniform float pitch;
out vec3 vColor;
out float vAcross;
out float vHalf;
const vec3 LIT = vec3(238.0, 238.0, 235.0) / 255.0;
const vec3 OFF = vec3(85.0, 85.0, 81.0) / 255.0;
const vec3 WIRE = vec3(187.0, 187.0, 181.0) / 255.0;
const vec3 ACCENT = vec3(223.0, 166.0, 75.0) / 255.0;
void main() {
  int kind = inst.w & 3;
  float arg = float((inst.w >> 2) - 64);
  vec4 p0 = texelFetch(points, ivec2(inst.y, inst.x), 0);
  vec4 p1 = texelFetch(points, ivec2(inst.y + 1, inst.x), 0);
  float off = kind == 1 ? arg * 0.5 * pitch : 0.0;
  vec2 a = (p0.xy + p0.zw * off) * view.z + view.xy;
  vec2 b = (p1.xy + p1.zw * off) * view.z + view.xy;
  vec2 d = b - a;
  float len = length(d);
  d = len > 1e-4 ? d / len : vec2(1.0, 0.0);
  vec2 n = vec2(-d.y, d.x);
  float widthG = kind == 0 ? 2.0 : kind == 1 ? 1.2 : kind == 2 ? 4.0 : arg;
  float halfW = max(widthG * view.z, 1.0) * 0.5;
  int v = gl_VertexID;
  float along = (v == 1 || v == 4 || v == 5) ? 1.0 : 0.0;
  float side = (v == 2 || v == 3 || v == 5) ? 1.0 : -1.0;
  float ext = halfW + 1.0;
  vec2 pos = mix(a, b, along) + d * (along * 2.0 - 1.0) * halfW + n * side * ext;
  vec2 clip = pos / size * 2.0 - 1.0;
  gl_Position = vec4(clip.x, -clip.y, 0.0, 1.0);
  vAcross = side * ext;
  vHalf = halfW;
  if (kind == 3) {
    vColor = ACCENT;
  } else if (kind == 2) {
    vColor = WIRE;
  } else {
    uint byteValue = texelFetch(bits, ivec2(inst.z >> 3, 0), 0).r;
    vColor = ((byteValue >> uint(inst.z & 7)) & 1u) == 1u ? LIT : OFF;
  }
}`;

const FRAGMENT = `#version 300 es
precision mediump float;
in vec3 vColor;
in float vAcross;
in float vHalf;
out vec4 color;
void main() {
  float a = clamp(vHalf + 0.5 - abs(vAcross), 0.0, 1.0);
  color = vec4(vColor * a, a);
}`;

function pack(edge: number, seg: number, lane: number, arg: number, kind: number, out: Int16Array, at: number) {
  out[at] = edge;
  out[at + 1] = seg;
  out[at + 2] = lane;
  out[at + 3] = (Math.round(arg) + 64) * 4 + kind;
}

/** The instances of all lanes (near) or of the bus centerlines (far). */
function buildInstances(wires: Wires, near: boolean): Int16Array {
  const { edges } = wires.graph;
  let count = 0;
  for (let i = 0; i < edges.length; i++) {
    const lanes = near ? edges[i].width : 1;
    count += lanes * (wires.counts[i] - 1);
  }
  const out = new Int16Array(count * 4);
  let at = 0;
  for (let i = 0; i < edges.length; i++) {
    const e = edges[i];
    const segs = wires.counts[i] - 1;
    for (let j = 0; j < (near ? e.width : 1); j++) {
      const kind = e.width === 1 ? KIND_BOOL : near ? KIND_LANE : KIND_FAR_BUS;
      const lane = kind === KIND_FAR_BUS ? -1 : e.lane0 + j;
      const off2 = kind === KIND_LANE ? 2 * j - (e.width - 1) : 0;
      for (let s = 0; s < segs; s++, at += 4) pack(i, s, lane, off2, kind, out, at);
    }
  }
  return out;
}

/** The name of the GPU that the browser uses. */
export function gpuName(gl: WebGL2RenderingContext): string {
  const info = gl.getExtension('WEBGL_debug_renderer_info');
  return String(gl.getParameter(info ? info.UNMASKED_RENDERER_WEBGL : gl.RENDERER));
}

function compile(gl: WebGL2RenderingContext, type: number, source: string): WebGLShader {
  const shader = gl.createShader(type)!;
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) throw new Error(`shader: ${gl.getShaderInfoLog(shader)}`);
  return shader;
}

class InstanceBuffer {
  readonly vao: WebGLVertexArrayObject;
  private readonly buffer: WebGLBuffer;
  count = 0;

  constructor(private readonly gl: WebGL2RenderingContext) {
    this.vao = gl.createVertexArray()!;
    this.buffer = gl.createBuffer()!;
    gl.bindVertexArray(this.vao);
    gl.bindBuffer(gl.ARRAY_BUFFER, this.buffer);
    gl.enableVertexAttribArray(0);
    gl.vertexAttribIPointer(0, 4, gl.SHORT, 8, 0);
    gl.vertexAttribDivisor(0, 1);
    gl.bindVertexArray(null);
  }

  upload(data: Int16Array) {
    const gl = this.gl;
    gl.bindBuffer(gl.ARRAY_BUFFER, this.buffer);
    gl.bufferData(gl.ARRAY_BUFFER, data, gl.STATIC_DRAW);
    this.count = data.length / 4;
  }
}

export class GlWires {
  private readonly gl: WebGL2RenderingContext;
  private readonly ctx: CanvasRenderingContext2D;
  private readonly program: WebGLProgram;
  private readonly uniforms: Record<'points' | 'bits' | 'view' | 'size' | 'pitch', WebGLUniformLocation | null>;
  private readonly pointsTex: WebGLTexture;
  private readonly bitsTex: WebGLTexture;
  private readonly near: InstanceBuffer;
  private readonly far: InstanceBuffer;
  private readonly selection: InstanceBuffer;
  private readonly badgeVal: Float64Array;
  private readonly badgeText: string[];
  private selected = -1;
  private width = 1;
  private height = 1;
  private dpr = 1;
  private dirty = true;
  private stale = false;
  private last = [NaN, NaN, NaN];
  private lastVersion = -1;

  constructor(
    canvas: HTMLCanvasElement,
    overlay: HTMLCanvasElement,
    private readonly wires: Wires,
    private readonly lanes: Lanes,
  ) {
    const gl = canvas.getContext('webgl2', { antialias: false, premultipliedAlpha: true });
    const ctx = overlay.getContext('2d');
    if (!gl || !ctx) throw new Error('WebGL2 or the 2D canvas is not available');
    this.gl = gl;
    this.ctx = ctx;
    const program = gl.createProgram()!;
    gl.attachShader(program, compile(gl, gl.VERTEX_SHADER, VERTEX));
    gl.attachShader(program, compile(gl, gl.FRAGMENT_SHADER, FRAGMENT));
    gl.linkProgram(program);
    if (!gl.getProgramParameter(program, gl.LINK_STATUS)) throw new Error(`program: ${gl.getProgramInfoLog(program)}`);
    this.program = program;
    const u = (name: string) => gl.getUniformLocation(program, name);
    this.uniforms = { points: u('points'), bits: u('bits'), view: u('view'), size: u('size'), pitch: u('pitch') };

    gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
    this.pointsTex = this.texture(0);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA32F, MAX_POINTS, wires.counts.length, 0, gl.RGBA, gl.FLOAT, wires.points);
    this.bitsTex = this.texture(1);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.R8UI, lanes.bits.length, 1, 0, gl.RED_INTEGER, gl.UNSIGNED_BYTE, lanes.bits);

    this.near = new InstanceBuffer(gl);
    this.far = new InstanceBuffer(gl);
    this.selection = new InstanceBuffer(gl);
    this.rebuildInstances();
    this.badgeVal = new Float64Array(wires.counts.length).fill(NaN);
    this.badgeText = new Array(wires.counts.length).fill('');
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
  }

  private texture(unit: number): WebGLTexture {
    const gl = this.gl;
    const tex = gl.createTexture()!;
    gl.activeTexture(gl.TEXTURE0 + unit);
    gl.bindTexture(gl.TEXTURE_2D, tex);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    return tex;
  }

  private rebuildInstances() {
    this.near.upload(buildInstances(this.wires, true));
    this.far.upload(buildInstances(this.wires, false));
    this.stale = false;
  }

  renderer(): string {
    return gpuName(this.gl);
  }

  /** Sets the canvas size in CSS pixels and the device pixel ratio. */
  resize(width: number, height: number, dpr: number) {
    this.width = Math.max(1, width);
    this.height = Math.max(1, height);
    this.dpr = dpr;
    for (const canvas of [this.gl.canvas as HTMLCanvasElement, this.ctx.canvas]) {
      canvas.width = Math.round(this.width * dpr);
      canvas.height = Math.round(this.height * dpr);
    }
    this.dirty = true;
  }

  /** Sends the new geometry of edge i to the GPU. */
  updateEdge(i: number, oldCount: number) {
    const gl = this.gl;
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, this.pointsTex);
    const row = this.wires.points.subarray(i * MAX_POINTS * 4, (i + 1) * MAX_POINTS * 4);
    gl.texSubImage2D(gl.TEXTURE_2D, 0, 0, i, MAX_POINTS, 1, gl.RGBA, gl.FLOAT, row);
    if (this.wires.counts[i] !== oldCount) this.stale = true;
    this.selected = this.selected === i ? -2 - i : this.selected;
    this.dirty = true;
  }

  setSelected(i: number) {
    this.selected = i < 0 ? -1 : -2 - i;
    this.dirty = true;
  }

  private selectionFor(near: boolean) {
    // A value under -1 marks a selection whose instances must be built again.
    const i = -2 - this.selected;
    if (this.selected < -1) {
      const segs = this.wires.counts[i] - 1;
      const data = new Int16Array(segs * 4);
      const width = this.wires.band(i, near) + SELECT_EXTRA;
      for (let s = 0; s < segs; s++) pack(i, s, -1, width, KIND_SELECT, data, s * 4);
      this.selection.upload(data);
      this.selected = i;
      this.selectedNear = near;
    }
  }

  private selectedNear = false;

  /** Draws when the view, the lane states or the geometry changed. */
  draw(tx: number, ty: number, zoom: number) {
    const near = zoom >= NEAR_ZOOM;
    const statesChanged = this.lanes.version !== this.lastVersion;
    const viewChanged = tx !== this.last[0] || ty !== this.last[1] || zoom !== this.last[2];
    if (!this.dirty && !statesChanged && !viewChanged) return;
    if (this.selected >= 0 && this.selectedNear !== near) this.selected = -2 - this.selected;
    this.selectionFor(near);
    if (this.stale) this.rebuildInstances();
    const gl = this.gl;
    if (statesChanged) {
      gl.activeTexture(gl.TEXTURE1);
      gl.bindTexture(gl.TEXTURE_2D, this.bitsTex);
      gl.texSubImage2D(gl.TEXTURE_2D, 0, 0, 0, this.lanes.bits.length, 1, gl.RED_INTEGER, gl.UNSIGNED_BYTE, this.lanes.bits);
    }
    gl.viewport(0, 0, gl.drawingBufferWidth, gl.drawingBufferHeight);
    gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT);
    gl.useProgram(this.program);
    gl.uniform1i(this.uniforms.points, 0);
    gl.uniform1i(this.uniforms.bits, 1);
    gl.uniform3f(this.uniforms.view, tx, ty, zoom);
    gl.uniform2f(this.uniforms.size, this.width, this.height);
    gl.uniform1f(this.uniforms.pitch, this.wires.graph.lanePitch);
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, this.pointsTex);
    gl.activeTexture(gl.TEXTURE1);
    gl.bindTexture(gl.TEXTURE_2D, this.bitsTex);
    const buffers = this.selected >= 0 ? [this.selection, near ? this.near : this.far] : [near ? this.near : this.far];
    for (const b of buffers) {
      gl.bindVertexArray(b.vao);
      gl.drawArraysInstanced(gl.TRIANGLES, 0, 6, b.count);
    }
    gl.bindVertexArray(null);
    this.drawBadges(tx, ty, zoom, near);
    this.last[0] = tx;
    this.last[1] = ty;
    this.last[2] = zoom;
    this.lastVersion = this.lanes.version;
    this.dirty = false;
  }

  private drawBadges(tx: number, ty: number, zoom: number, near: boolean) {
    const { ctx, wires } = this;
    const { edges } = wires.graph;
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, ctx.canvas.width, ctx.canvas.height);
    ctx.setTransform(this.dpr * zoom, 0, 0, this.dpr * zoom, this.dpr * tx, this.dpr * ty);
    ctx.font = `${BADGE_FONT_SIZE}px ${BADGE_FONT}`;
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    const left = -tx / zoom;
    const top = -ty / zoom;
    const right = left + this.width / zoom;
    const bottom = top + this.height / zoom;
    for (let i = 0; i < edges.length; i++) {
      if (!near && edges[i].width === 1) continue;
      const x = wires.mid[i * 2];
      const y = wires.badgeY(i, near);
      const bw = wires.badgeWidth(i, near);
      if (x + bw < left || x - bw > right || y + BADGE_HEIGHT < top || y - BADGE_HEIGHT > bottom) continue;
      let text: string;
      if (near) {
        const v = wires.value(i, this.lanes.bits);
        if (v !== this.badgeVal[i]) {
          this.badgeVal[i] = v;
          this.badgeText[i] = wires.badgeText(i, true, v);
        }
        text = this.badgeText[i];
      } else {
        text = wires.badgeText(i, false, 0);
      }
      ctx.fillStyle = BADGE_BG;
      ctx.beginPath();
      ctx.roundRect(x - bw / 2, y - BADGE_HEIGHT / 2, bw, BADGE_HEIGHT, 3);
      ctx.fill();
      ctx.fillStyle = BADGE_TEXT;
      ctx.fillText(text, x, y + 0.5);
    }
  }
}

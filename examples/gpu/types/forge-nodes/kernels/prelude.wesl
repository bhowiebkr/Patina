// ---- prelude ----
// Shared by every node kernel; appended after the node's file and its generated accessors.
// Colours stay sRGB-encoded; gray values are linear 0..1.

struct Info {
    // Width, height, pass index, pass count.
    size: vec4<u32>,
    // x: mesh atlas, y: mesh map side, z: output layers, w: host bit0 / updated defaults bit8.
    domain: vec4<u32>,
    // Per input: x linked, y first layer, z read mode (0 load, 1 resample, 2 triplanar),
    // w source socket (0 gray, 1 colour, 2 material).
    inputs: array<vec4<u32>, 16>,
    // x: triplanar scale, y: blend sharpness.
    triplanar: vec4<f32>,
    // The mesh's bounding box: centre (xyz), largest side (w).
    bounds: vec4<f32>,
}

// A material travels as three layers: base colour (a: opacity); roughness, metallic, height,
// ambient occlusion; tangent-space normal (0..1, OpenGL convention, +Y up).
struct Material {
    basecolor: vec4<f32>,
    roughness: f32,
    metallic: f32,
    height: f32,
    ao: f32,
    normal: vec3<f32>,
}

@group(0) @binding(0) var<uniform> info: Info;
@group(0) @binding(1) var<storage, read> values: array<f32>;
@group(0) @binding(2) var samp: sampler;
// Object-space position (Y up), w = coverage (1 covered, 0.5 dilated, 0 empty).
@group(0) @binding(3) var map_pos: texture_2d<f32>;
// Object-space normal, w = curvature (0.5 flat, above convex).
@group(0) @binding(4) var map_nrm: texture_2d<f32>;
// r: ambient occlusion, g: thickness, b: part index, a: near other part.
@group(0) @binding(5) var map_extras: texture_2d<f32>;
// Multipass nodes: the previous pass's scratch and this pass's.
@group(0) @binding(6) var scratch_in: texture_2d<f32>;
@group(0) @binding(7) var scratch_out: texture_storage_2d<rgba32float, write>;
@group(0) @binding(8) var out_tex: texture_storage_2d_array<rgba32float, write>;
// `stats` nodes: zeroed before the first pass.
@group(0) @binding(9) var<storage, read_write> stats: array<atomic<u32>, 5120>;
// `atoms: N` nodes: N atomic u32 planes of one per pixel, zeroed before the first pass.
@group(0) @binding(26) var<storage, read_write> atoms: array<atomic<u32>>;
// Per part: CAD base colour (linear rgb), w = colour group (index into `@colors`).
@group(0) @binding(27) var<storage, read> part_table: array<vec4<f32>>;
@group(0) @binding(10) var input0: texture_2d_array<f32>;
@group(0) @binding(11) var input1: texture_2d_array<f32>;
@group(0) @binding(12) var input2: texture_2d_array<f32>;
@group(0) @binding(13) var input3: texture_2d_array<f32>;
@group(0) @binding(14) var input4: texture_2d_array<f32>;
@group(0) @binding(15) var input5: texture_2d_array<f32>;
@group(0) @binding(16) var input6: texture_2d_array<f32>;
@group(0) @binding(17) var input7: texture_2d_array<f32>;
@group(0) @binding(18) var input8: texture_2d_array<f32>;
@group(0) @binding(19) var input9: texture_2d_array<f32>;
@group(0) @binding(20) var input10: texture_2d_array<f32>;
@group(0) @binding(21) var input11: texture_2d_array<f32>;
@group(0) @binding(22) var input12: texture_2d_array<f32>;
@group(0) @binding(23) var input13: texture_2d_array<f32>;
@group(0) @binding(24) var input14: texture_2d_array<f32>;
@group(0) @binding(25) var input15: texture_2d_array<f32>;

const LUMA = vec3<f32>(0.2126, 0.7152, 0.0722);
const PI = 3.14159265358979;
const TAU = 6.28318530717959;

// ---- coordinates and passes ----

fn size() -> vec2<i32> {
    return vec2<i32>(info.size.xy);
}

fn size_f() -> vec2<f32> {
    return vec2<f32>(info.size.xy);
}

// Texel centre in 0..1.
fn uv(p: vec2<i32>) -> vec2<f32> {
    return (vec2<f32>(p) + 0.5) / size_f();
}

// Texel coordinates wrapped onto the tile.
fn wrap(p: vec2<i32>) -> vec2<i32> {
    let s = size();
    return ((p % s) + s) % s;
}

fn texel_of(uv: vec2<f32>) -> vec2<i32> {
    return wrap(vec2<i32>(floor(uv * size_f())));
}

fn pass_index() -> u32 {
    return info.size.z;
}

fn passes() -> u32 {
    return info.size.w;
}

fn last_pass() -> bool {
    return info.size.z + 1u == info.size.w;
}

fn is_mesh() -> bool {
    return info.domain.x != 0u;
}

fn scratch_load(p: vec2<i32>) -> vec4<f32> {
    return textureLoad(scratch_in, wrap(p), 0);
}

fn scratch_store(p: vec2<i32>, v: vec4<f32>) {
    textureStore(scratch_out, p, v);
}

// ---- mesh maps ----

// The mesh map texel under pixel `p`; a UV-space node smaller than the maps reads the texel at
// its pixel's centre.
fn mesh_texel(p: vec2<i32>) -> vec2<i32> {
    let map = i32(info.domain.y);
    let q = (p * map + map / 2) / i32(info.size.x);
    return clamp(q, vec2<i32>(0), vec2<i32>(map - 1));
}

fn mesh_position(p: vec2<i32>) -> vec3<f32> {
    return textureLoad(map_pos, mesh_texel(p), 0).xyz;
}

// The position in the bounding box scaled by its largest side, centred on 0.5 (a
// position map): 0..1 along the largest side.
fn mesh_unit_position(p: vec2<i32>) -> vec3<f32> {
    return (mesh_position(p) - info.bounds.xyz) / info.bounds.w + vec3<f32>(0.5);
}

fn mesh_covered(p: vec2<i32>) -> bool {
    return textureLoad(map_pos, mesh_texel(p), 0).w > 0.25;
}

fn mesh_normal(p: vec2<i32>) -> vec3<f32> {
    return textureLoad(map_nrm, mesh_texel(p), 0).xyz;
}

fn mesh_curvature(p: vec2<i32>) -> f32 {
    return textureLoad(map_nrm, mesh_texel(p), 0).w;
}

fn mesh_ao(p: vec2<i32>) -> f32 {
    return textureLoad(map_extras, mesh_texel(p), 0).r;
}

fn mesh_thickness(p: vec2<i32>) -> f32 {
    return textureLoad(map_extras, mesh_texel(p), 0).g;
}

// Distance to the nearest different part: 0 touching, 1 at a tenth of the bounds' largest side or
// farther, 1 on a mesh of one part.
fn mesh_near(p: vec2<i32>) -> f32 {
    return textureLoad(map_extras, mesh_texel(p), 0).a;
}

// Index of the part (into the asset's part list) the texel belongs to.
fn mesh_part(p: vec2<i32>) -> i32 {
    return i32(round(textureLoad(map_extras, mesh_texel(p), 0).b));
}

fn mesh_part_row(p: vec2<i32>) -> vec4<f32> {
    let last = i32(arrayLength(&part_table)) - 1;
    return part_table[clamp(mesh_part(p), 0, last)];
}

// The part's CAD base colour, linear.
fn mesh_cad_color(p: vec2<i32>) -> vec3<f32> {
    return mesh_part_row(p).rgb;
}

// The part's CAD colour group: parts with the same CAD colour share it (index into `@colors`).
fn mesh_cad_group(p: vec2<i32>) -> i32 {
    return i32(round(mesh_part_row(p).w));
}

// ---- inputs ----

fn triplanar(t: texture_2d_array<f32>, p: vec2<i32>, layer: i32) -> vec4<f32> {
    let q = mesh_position(p) * info.triplanar.x;
    let n = mesh_normal(p);
    var w = pow(abs(n), vec3<f32>(info.triplanar.y));
    w /= max(w.x + w.y + w.z, 1e-5);
    return textureSampleLevel(t, samp, q.zy, layer, 0.0) * w.x
        + textureSampleLevel(t, samp, q.xz, layer, 0.0) * w.y
        + textureSampleLevel(t, samp, q.xy, layer, 0.0) * w.z;
}

// Layer `layer` of input `slot` at this node's texel `p`.
fn fetch(t: texture_2d_array<f32>, slot: u32, p: vec2<i32>, layer: u32) -> vec4<f32> {
    let i = info.inputs[slot];
    let l = i32(i.y + layer);
    switch i.z {
        case 0u: {
            return textureLoad(t, p, l, 0);
        }
        case 1u: {
            return textureSampleLevel(t, samp, uv(p), l, 0.0);
        }
        default: {
            return triplanar(t, p, l);
        }
    }
}

// A colour read through a grayscale input converts by luminance.
fn to_gray(c: vec4<f32>, slot: u32) -> f32 {
    // Grayscale images can travel through a Color socket. A weighted dot can
    // round equal RGB channels by an ulp, changing subsequent fixed-point coordinates.
    if c.r == c.g && c.r == c.b {
        return c.r;
    }
    return select(c.r, dot(c.rgb, LUMA), info.inputs[slot].w == 1u);
}

fn material_default() -> Material {
    return Material(vec4<f32>(0.5, 0.5, 0.5, 1.0), 0.5, 0.0, 0.5, 1.0, vec3<f32>(0.5, 0.5, 1.0));
}

fn material_pack(a: vec4<f32>, b: vec4<f32>, c: vec4<f32>) -> Material {
    return Material(a, b.x, b.y, b.z, b.w, c.xyz);
}

fn material_fetch(t: texture_2d_array<f32>, slot: u32, p: vec2<i32>) -> Material {
    return material_pack(fetch(t, slot, p, 0u), fetch(t, slot, p, 1u), fetch(t, slot, p, 2u));
}

fn material_sample(t: texture_2d_array<f32>, slot: u32, uv: vec2<f32>) -> Material {
    let l = i32(info.inputs[slot].y);
    return material_pack(
        textureSampleLevel(t, samp, uv, l, 0.0),
        textureSampleLevel(t, samp, uv, l + 1, 0.0),
        textureSampleLevel(t, samp, uv, l + 2, 0.0),
    );
}

fn material_mix(a: Material, b: Material, t: f32) -> Material {
    return Material(
        mix(a.basecolor, b.basecolor, t),
        mix(a.roughness, b.roughness, t),
        mix(a.metallic, b.metallic, t),
        mix(a.height, b.height, t),
        mix(a.ao, b.ao, t),
        mix(a.normal, b.normal, t),
    );
}

// ---- outputs ----

fn store_out(p: vec2<i32>, layer: i32, v: vec4<f32>) {
    textureStore(out_tex, p, layer, v);
}

fn store_gray(p: vec2<i32>, layer: u32, g: f32) {
    store_out(p, i32(layer), vec4<f32>(g, g, g, 1.0));
}

fn store_color(p: vec2<i32>, layer: u32, c: vec4<f32>) {
    store_out(p, i32(layer), c);
}

fn store_material(p: vec2<i32>, layer: u32, m: Material) {
    store_out(p, i32(layer), m.basecolor);
    store_out(p, i32(layer) + 1, vec4<f32>(m.roughness, m.metallic, m.height, m.ao));
    store_out(p, i32(layer) + 2, vec4<f32>(m.normal, 1.0));
}

// ---- colour ----

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    return select(pow((c + 0.055) / 1.055, vec3<f32>(2.4)), c / 12.92, c <= vec3<f32>(0.04045));
}

fn linear_to_srgb(c: vec3<f32>) -> vec3<f32> {
    let k = max(c, vec3<f32>(0.0));
    return select(1.055 * pow(k, vec3<f32>(1.0 / 2.4)) - 0.055, k * 12.92, k <= vec3<f32>(0.0031308));
}

fn luminance(c: vec3<f32>) -> f32 {
    return dot(c, LUMA);
}

fn rgb_to_hsl(c: vec3<f32>) -> vec3<f32> {
    let hi = max(c.r, max(c.g, c.b));
    let lo = min(c.r, min(c.g, c.b));
    let l = 0.5 * (hi + lo);
    let d = hi - lo;
    if d < 1e-6 {
        return vec3<f32>(0.0, 0.0, l);
    }
    let s = d / (1.0 - abs(2.0 * l - 1.0) + 1e-6);
    var h: f32;
    if hi == c.r {
        h = (c.g - c.b) / d + select(0.0, 6.0, c.g < c.b);
    } else if hi == c.g {
        h = (c.b - c.r) / d + 2.0;
    } else {
        h = (c.r - c.g) / d + 4.0;
    }
    return vec3<f32>(h / 6.0, s, l);
}

fn hsl_to_rgb(c: vec3<f32>) -> vec3<f32> {
    let k = (vec3<f32>(0.0, 8.0, 4.0) + c.x * 12.0) % vec3<f32>(12.0);
    let a = c.y * min(c.z, 1.0 - c.z);
    return c.z - a * clamp(min(k - 3.0, 9.0 - k), vec3<f32>(-1.0), vec3<f32>(1.0));
}

// ---- gradients and curves (packed by the param types) ----

fn gradient_stop(off: u32, i: u32) -> vec4<f32> {
    return gradient_stop_stride(off, i, 5u);
}

fn gradient_stop_stride(off: u32, i: u32, stride: u32) -> vec4<f32> {
    let b = off + 2u + i * stride;
    return vec4<f32>(values[b], values[b + 1u], values[b + 2u], values[b + 3u]);
}

fn gradient_step(off: u32, t: f32) -> vec4<f32> {
    return gradient_step_stride(off, t, 5u);
}

fn gradient_step_stride(off: u32, t: f32, stride: u32) -> vec4<f32> {
    let count = u32(values[off]);
    if count == 0u {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    var result = gradient_stop_stride(off, 0u, stride);
    for (var i = 1u; i < count; i++) {
        if t < values[off + 1u + i * stride] {
            break;
        }
        result = gradient_stop_stride(off, i, stride);
    }
    return result;
}

// `ease`: smoothstep between stops, as legacy gradients without midpoints.
fn gradient_eval(off: u32, t: f32, ease: bool) -> vec4<f32> {
    return gradient_eval_stride(off, t, ease, 5u);
}

fn gradient_eval_stride(off: u32, t: f32, ease: bool, stride: u32) -> vec4<f32> {
    let count = u32(values[off]);
    if count == 0u {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    var prev_pos = values[off + 1u];
    var prev = gradient_stop_stride(off, 0u, stride);
    if t <= prev_pos {
        return prev;
    }
    for (var i = 1u; i < count; i++) {
        let pos = values[off + 1u + i * stride];
        let c = gradient_stop_stride(off, i, stride);
        if t <= pos {
            let f = clamp((t - prev_pos) / max(pos - prev_pos, 1e-6), 0.0, 1.0);
            return mix(prev, c, select(f, f * f * (3.0 - 2.0 * f), ease));
        }
        prev_pos = pos;
        prev = c;
    }
    return prev;
}

// Modern Flat Tangents: each segment's left key biases its normalized coordinate
// quadratically, then applies smoothstep. This is not a cubic Bezier x inversion.
fn gradient_flat(off: u32, t: f32) -> vec4<f32> {
    let count = u32(values[off]);
    if count == 0u { return vec4<f32>(0.0, 0.0, 0.0, 1.0); }
    var prev_pos = values[off + 1u];
    var prev = gradient_stop_stride(off, 0u, 6u);
    if t <= prev_pos { return prev; }
    for (var i = 1u; i < count; i++) {
        let pos = values[off + 1u + i * 6u];
        let c = gradient_stop_stride(off, i, 6u);
        if t <= pos {
            let f = clamp((t - prev_pos) / max(pos - prev_pos, 1e-6), 0.0, 1.0);
            let midpoint = values[off + 6u + (i - 1u) * 6u];
            let g = ((midpoint * f - midpoint) + 1.0) * f;
            return prev + (c - prev) * ((3.0 - (g + g)) * g * g);
        }
        prev_pos = pos;
        prev = c;
    }
    return prev;
}

fn bezier(a: f32, b: f32, c: f32, d: f32, t: f32) -> f32 {
    let s = 1.0 - t;
    return s * s * s * a + 3.0 * s * s * t * b + 3.0 * s * t * t * c + t * t * t * d;
}

fn bezier_slope(a: f32, b: f32, c: f32, d: f32, t: f32) -> f32 {
    let s = 1.0 - t;
    return 3.0 * s * s * (b - a) + 6.0 * s * t * (c - b) + 3.0 * t * t * (d - c);
}

fn curve_point(off: u32, i: u32, k: u32) -> vec2<f32> {
    let b = off + 1u + i * 6u + k * 2u;
    return vec2<f32>(values[b], values[b + 1u]);
}

// Points (x, y) with absolute left and right bezier handles; flat beyond the end points.
fn curve_eval(off: u32, x: f32) -> f32 {
    let count = u32(values[off]);
    if count == 0u {
        return x;
    }
    if x <= curve_point(off, 0u, 0u).x {
        return curve_point(off, 0u, 0u).y;
    }
    for (var i = 0u; i + 1u < count; i++) {
        let p0 = curve_point(off, i, 0u);
        let p3 = curve_point(off, i + 1u, 0u);
        if x > p3.x {
            continue;
        }
        let p1 = vec2<f32>(clamp(curve_point(off, i, 2u).x, p0.x, p3.x), curve_point(off, i, 2u).y);
        let p2 = vec2<f32>(clamp(curve_point(off, i + 1u, 1u).x, p0.x, p3.x), curve_point(off, i + 1u, 1u).y);
        var t = clamp((x - p0.x) / max(p3.x - p0.x, 1e-6), 0.0, 1.0);
        var lo = 0.0;
        var hi = 1.0;
        for (var k = 0; k < 12; k++) {
            let e = bezier(p0.x, p1.x, p2.x, p3.x, t) - x;
            if abs(e) < 1e-5 {
                break;
            }
            if e > 0.0 {
                hi = t;
            } else {
                lo = t;
            }
            let d = bezier_slope(p0.x, p1.x, p2.x, p3.x, t);
            var next = t - e / select(d, 1.0, d == 0.0);
            if d == 0.0 || next <= lo || next >= hi {
                next = 0.5 * (lo + hi);
            }
            t = next;
        }
        return bezier(p0.y, p1.y, p2.y, p3.y, t);
    }
    return curve_point(off, count - 1u, 0u).y;
}

// ---- stats: floats as order-preserving u32 keys for atomics ----

fn float_key(f: f32) -> u32 {
    let u = bitcast<u32>(f);
    return select(u | 0x80000000u, ~u, (u & 0x80000000u) != 0u);
}

fn key_float(k: u32) -> f32 {
    let u = select(~k, k & 0x7fffffffu, (k & 0x80000000u) != 0u);
    return bitcast<f32>(u);
}

fn stats_max(slot: u32, v: f32) {
    atomicMax(&stats[slot], float_key(v));
}

fn stats_min(slot: u32, v: f32) {
    atomicMax(&stats[slot], ~float_key(v));
}

fn stats_add(slot: u32, v: u32) {
    atomicAdd(&stats[slot], v);
}

fn stats_get(slot: u32) -> u32 {
    return atomicLoad(&stats[slot]);
}

fn stats_get_max(slot: u32) -> f32 {
    return key_float(atomicLoad(&stats[slot]));
}

fn stats_get_min(slot: u32) -> f32 {
    return key_float(~atomicLoad(&stats[slot]));
}

// ---- atoms (`atoms: N` nodes): plane `plane`'s slot for pixel p (wrapped) ----
fn atom_slot(plane: u32, p: vec2<i32>) -> u32 {
    let w = wrap(p);
    return plane * u32(size().x * size().y) + u32(w.y * size().x + w.x);
}

// ---- hashing and noise ----

fn pcg(v: u32) -> u32 {
    let s = v * 747796405u + 2891336453u;
    let w = ((s >> ((s >> 28u) + 4u)) ^ s) * 277803737u;
    return (w >> 22u) ^ w;
}

// Uniform 0..1 from an integer cell and a seed.
fn hash21(q: vec2<i32>, seed: u32) -> f32 {
    let u = bitcast<vec2<u32>>(q);
    return f32(pcg(u.x ^ pcg(u.y ^ pcg(seed)))) / 4294967295.0;
}

fn hash22(q: vec2<i32>, seed: u32) -> vec2<f32> {
    let u = bitcast<vec2<u32>>(q);
    let h = pcg(u.x ^ pcg(u.y ^ pcg(seed)));
    return vec2<f32>(f32(h), f32(pcg(h))) / 4294967295.0;
}

// Signed -1..1 per component.
fn hash3(q: vec3<i32>) -> vec3<f32> {
    var v = bitcast<vec3<u32>>(q) * 1664525u + 1013904223u;
    v.x += v.y * v.z;
    v.y += v.z * v.x;
    v.z += v.x * v.y;
    v ^= v >> vec3<u32>(16u);
    v.x += v.y * v.z;
    v.y += v.z * v.x;
    v.z += v.x * v.y;
    return vec3<f32>(v) * (2.0 / 4294967295.0) - 1.0;
}

fn noise_corner(i: vec3<i32>, f: vec3<f32>, o: vec3<i32>) -> f32 {
    return dot(hash3(i + o), f - vec3<f32>(o));
}

fn fade(f: vec3<f32>) -> vec3<f32> {
    return f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
}

// 3D gradient noise, about -1..1.
fn gradient_noise(x: vec3<f32>) -> f32 {
    let cell = floor(x);
    let i = vec3<i32>(cell);
    let f = x - cell;
    let u = fade(f);
    let x00 = mix(noise_corner(i, f, vec3<i32>(0, 0, 0)), noise_corner(i, f, vec3<i32>(1, 0, 0)), u.x);
    let x10 = mix(noise_corner(i, f, vec3<i32>(0, 1, 0)), noise_corner(i, f, vec3<i32>(1, 1, 0)), u.x);
    let x01 = mix(noise_corner(i, f, vec3<i32>(0, 0, 1)), noise_corner(i, f, vec3<i32>(1, 0, 1)), u.x);
    let x11 = mix(noise_corner(i, f, vec3<i32>(0, 1, 1)), noise_corner(i, f, vec3<i32>(1, 1, 1)), u.x);
    return mix(mix(x00, x10, u.y), mix(x01, x11, u.y), u.z);
}

// Tileable 2D gradient noise with `period` cells across the tile, about -1..1.
fn tile_gradient_noise(uv: vec2<f32>, period: i32, seed: u32) -> f32 {
    let x = uv * f32(period);
    let cell = floor(x);
    let f = x - cell;
    let i = vec2<i32>(cell);
    let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    var c: array<f32, 4>;
    for (var k = 0; k < 4; k++) {
        let o = vec2<i32>(k & 1, k >> 1u);
        let q = ((i + o) % period + period) % period;
        let a = hash21(q, seed) * TAU;
        c[k] = dot(vec2<f32>(cos(a), sin(a)), f - vec2<f32>(o));
    }
    return mix(mix(c[0], c[1], u.x), mix(c[2], c[3], u.x), u.y) * 1.41421356;
}

// Tileable 2D value noise, 0..1.
fn tile_value_noise(uv: vec2<f32>, period: i32, seed: u32) -> f32 {
    let x = uv * f32(period);
    let cell = floor(x);
    let f = x - cell;
    let i = vec2<i32>(cell);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash21((i % period + period) % period, seed);
    let b = hash21(((i + vec2<i32>(1, 0)) % period + period) % period, seed);
    let c = hash21(((i + vec2<i32>(0, 1)) % period + period) % period, seed);
    let d = hash21(((i + vec2<i32>(1, 1)) % period + period) % period, seed);
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// Tiling noise shared by the noise and pattern nodes; appended to the prelude.
//
// Positions are in cells (uv * period) and repeat every `period` cells, so every noise tiles.
// Disorder steps through a third, untiled axis z; Disorder anisotropy turns part of that step
// into an in-plane drift along an angle (turns).

fn cell_wrap(c: vec2<i32>, period: vec2<i32>) -> vec2<i32> {
    return ((c % period) + period) % period;
}

// Wraps cell c every `period` cells on each axis; an axis of period 0 does not wrap.
fn wrap3(c: vec3<i32>, period: vec3<i32>) -> vec3<i32> {
    let m = max(period, vec3<i32>(1));
    return select(c, ((c % m) + m) % m, period > vec3<i32>(0));
}

fn hash31(q: vec3<i32>, seed: u32) -> f32 {
    let u = bitcast<vec3<u32>>(q);
    return f32(pcg(u.x ^ pcg(u.y ^ pcg(u.z ^ pcg(seed))))) / 4294967295.0;
}

fn hash33(q: vec3<i32>, seed: u32) -> vec3<f32> {
    let u = bitcast<vec3<u32>>(q);
    let h = pcg(u.x ^ pcg(u.y ^ pcg(u.z ^ pcg(seed))));
    let h2 = pcg(h);
    return vec3<f32>(f32(h), f32(h2), f32(pcg(h2))) / 4294967295.0;
}

// The seed a node passes to the noise functions: its random seed, offset per use.
fn noise_seed(randomseed: i32, salt: u32) -> u32 {
    return pcg(u32(randomseed) * 1013u + salt);
}

// Cells across for a noise of `scale` cells; with Non-square expansion the longer side of a
// non-square image gets proportionally more, so cells stay square.
fn noise_period(scale: i32, nonsquare: bool) -> vec2<i32> {
    let s = max(scale, 1);
    let n = size_f();
    if !nonsquare || n.x == n.y {
        return vec2<i32>(s);
    }
    let k = n / min(n.x, n.y);
    return max(vec2<i32>(round(f32(s) * k)), vec2<i32>(1));
}

// (drift x, drift y, z) in cells for Disorder `amount * speed`.
fn disorder_shift(amount: f32, speed: f32, aniso: f32, angle: f32) -> vec3<f32> {
    let t = amount * speed;
    let a = angle * TAU;
    let k = clamp(aniso, 0.0, 1.0);
    return vec3<f32>(vec2<f32>(cos(a), -sin(a)) * t * k, t * (1.0 - k));
}

// Cell position of pixel p for a noise `period` cells across, offset by a tile offset (in
// image widths) and a disorder shift.
fn noise_pos(p: vec2<i32>, period: vec2<i32>, offset: vec2<f32>, shift: vec3<f32>) -> vec3<f32> {
    let x = (uv(p) + offset) * vec2<f32>(period) + shift.xy;
    return vec3<f32>(x, shift.z);
}

// Pixel p in image widths, offset and drifted, for the fbm functions (which multiply by each
// octave's period).
fn noise_uv(p: vec2<i32>, period: vec2<i32>, offset: vec2<f32>, shift: vec3<f32>) -> vec2<f32> {
    return uv(p) + offset + shift.xy / vec2<f32>(period);
}

// Folds x into 0..1 and back; tri_wave(h + c) of a uniform h stays uniform for any shift c,
// which is how random values animate with Disorder.
fn tri_wave(x: f32) -> f32 {
    return abs(fract(x) * 2.0 - 1.0);
}

// A random 0..1 value `h` that wanders as Disorder `t` grows, at a per-element `rate` (0..1).
fn wander(h: f32, rate: f32, t: f32) -> f32 {
    return tri_wave(h + t * (0.5 + 0.5 * rate));
}

// v turned counter-clockwise by `turns`.
fn rotate2(v: vec2<f32>, turns: f32) -> vec2<f32> {
    let a = turns * TAU;
    let c = cos(a);
    let s = sin(a);
    return vec2<f32>(c * v.x - s * v.y, s * v.x + c * v.y);
}

fn grad_corner(i: vec3<i32>, o: vec3<i32>, f: vec3<f32>, period: vec3<i32>, seed: u32) -> f32 {
    let g = hash33(wrap3(i + o, period), seed) * 2.0 - 1.0;
    return dot(g, f - vec3<f32>(o));
}

// Gradient (Perlin) noise tiling in x and y, about -1..1.
fn perlin3(x: vec3<f32>, period: vec2<i32>, seed: u32) -> f32 {
    return perlin3p(x, vec3<i32>(period, 0), seed);
}

// Gradient noise tiling every `period` cells per axis (0: that axis does not tile).
fn perlin3p(x: vec3<f32>, period: vec3<i32>, seed: u32) -> f32 {
    let cell = floor(x);
    let i = vec3<i32>(cell);
    let f = x - cell;
    let u = fade(f);
    let a = mix(grad_corner(i, vec3<i32>(0, 0, 0), f, period, seed), grad_corner(i, vec3<i32>(1, 0, 0), f, period, seed), u.x);
    let b = mix(grad_corner(i, vec3<i32>(0, 1, 0), f, period, seed), grad_corner(i, vec3<i32>(1, 1, 0), f, period, seed), u.x);
    let c = mix(grad_corner(i, vec3<i32>(0, 0, 1), f, period, seed), grad_corner(i, vec3<i32>(1, 0, 1), f, period, seed), u.x);
    let d = mix(grad_corner(i, vec3<i32>(0, 1, 1), f, period, seed), grad_corner(i, vec3<i32>(1, 1, 1), f, period, seed), u.x);
    return mix(mix(a, b, u.y), mix(c, d, u.y), u.z) * 1.6;
}

fn value_corner(i: vec3<i32>, o: vec3<i32>, period: vec2<i32>, seed: u32) -> f32 {
    let c = i + o;
    return hash31(vec3<i32>(cell_wrap(c.xy, period), c.z), seed);
}

// Value noise tiling in x and y, 0..1; `smoothness` 0 is linear, 1 is smoothstep between cells.
fn value3(x: vec3<f32>, period: vec2<i32>, seed: u32, smoothness: f32) -> f32 {
    let cell = floor(x);
    let i = vec3<i32>(cell);
    let f = x - cell;
    let u = mix(f, f * f * (3.0 - 2.0 * f), smoothness);
    let a = mix(value_corner(i, vec3<i32>(0, 0, 0), period, seed), value_corner(i, vec3<i32>(1, 0, 0), period, seed), u.x);
    let b = mix(value_corner(i, vec3<i32>(0, 1, 0), period, seed), value_corner(i, vec3<i32>(1, 1, 0), period, seed), u.x);
    let c = mix(value_corner(i, vec3<i32>(0, 0, 1), period, seed), value_corner(i, vec3<i32>(1, 0, 1), period, seed), u.x);
    let d = mix(value_corner(i, vec3<i32>(0, 1, 1), period, seed), value_corner(i, vec3<i32>(1, 1, 1), period, seed), u.x);
    return mix(mix(a, b, u.y), mix(c, d, u.y), u.z);
}

const FBM_VALUE: i32 = 0;
const FBM_VALUE_LINEAR: i32 = 1;
const FBM_PERLIN: i32 = 2;
const FBM_RIDGED: i32 = 3;

// One octave of `kind` around 0 (about -0.5..0.5).
fn octave(kind: i32, x: vec3<f32>, period: vec2<i32>, seed: u32) -> f32 {
    if kind == FBM_PERLIN {
        return 0.5 * perlin3(x, period, seed);
    }
    if kind == FBM_RIDGED {
        return 0.5 - abs(perlin3(x, period, seed));
    }
    return value3(x, period, seed, select(1.0, 0.0, kind == FBM_VALUE_LINEAR)) - 0.5;
}

// Octaves from `period` cells across up, each twice as fine and `roughness` times as strong as
// the one before, skipping the first `skip`; 0..1, mean 0.5. Octaves stop below two pixels a
// cell.
fn fbm_levels(q: vec2<f32>, z: f32, period: vec2<i32>, skip: i32, octaves: i32, roughness: f32, kind: i32, seed: u32) -> f32 {
    var sum = 0.0;
    var total = 0.0;
    var amp = 1.0;
    var n = period;
    for (var o = 0; o < skip + octaves && f32(n.x) <= size_f().x * 0.5; o += 1) {
        if o >= skip {
            let x = vec3<f32>(q * vec2<f32>(n), z * f32(1 << u32(o)) + f32(o) * 17.0);
            sum += octave(kind, x, n, seed + u32(o) * 7919u) * amp;
            total += amp;
            amp *= roughness;
        }
        n *= 2;
    }
    return 0.5 + sum / max(total, 1e-4);
}

fn fbm(q: vec2<f32>, z: f32, period: vec2<i32>, octaves: i32, roughness: f32, kind: i32, seed: u32) -> f32 {
    return fbm_levels(q, z, period, 0, octaves, roughness, kind, seed);
}

// Raises contrast about 0.5: 0 leaves v, 1 is a hard threshold.
fn contrast(v: f32, amount: f32) -> f32 {
    let k = clamp(amount, 0.0, 0.999);
    return clamp((v - 0.5) / (1.0 - k) + 0.5, 0.0, 1.0);
}

// A Gaussian-distributed smooth noise about 0 with standard deviation about 1/6: the sum of four
// gradient noises (central limit).
fn gaussian3(x: vec3<f32>, period: vec2<i32>, seed: u32) -> f32 {
    var s = 0.0;
    for (var k = 0u; k < 4u; k += 1u) {
        s += perlin3(x + vec3<f32>(0.0, 0.0, f32(k) * 31.7), period, seed + k * 104729u);
    }
    return s * 0.125;
}

// ---- cellular (Worley / Voronoi) ----

const METRIC_EUCLIDEAN: i32 = 0;
const METRIC_MANHATTAN: i32 = 1;
const METRIC_CHEBYSHEV: i32 = 2;
const METRIC_MINKOWSKI: i32 = 3;

fn metric_length(d: vec2<f32>, metric: i32, p: f32) -> f32 {
    return metric_length3(vec3<f32>(d, 0.0), metric, p);
}

fn metric_length3(d: vec3<f32>, metric: i32, p: f32) -> f32 {
    let a = abs(d);
    if metric == METRIC_MANHATTAN {
        return a.x + a.y + a.z;
    }
    if metric == METRIC_CHEBYSHEV {
        return max(max(a.x, a.y), a.z);
    }
    if metric == METRIC_MINKOWSKI {
        let k = max(p, 0.1);
        return pow(pow(a.x, k) + pow(a.y, k) + pow(a.z, k), 1.0 / k);
    }
    return length(d);
}

// Feature point of cell c (unwrapped cell coordinates), jittered inside its cell; `t` moves it
// smoothly (Disorder).
fn cell_point(c: vec2<i32>, period: vec2<i32>, jitter: f32, t: f32, seed: u32) -> vec2<f32> {
    let w = cell_wrap(c, period);
    let h = hash22(w, seed);
    let v = 0.5 + 0.5 * hash22(w, seed ^ 0x9e3779b9u);
    let j = vec2<f32>(tri_wave(h.x + t * v.x), tri_wave(h.y + t * v.y)) - 0.5;
    return vec2<f32>(c) + 0.5 + j * jitter;
}

struct Cells {
    f1: f32,
    f2: f32,
    // Wrapped cell of the nearest and second-nearest points.
    id1: vec2<i32>,
    id2: vec2<i32>,
    // Nearest point, in the same (unwrapped) cell coordinates as x.
    p1: vec2<f32>,
}

// Nearest and second-nearest feature distances around x (in cells).
fn cells(x: vec2<f32>, period: vec2<i32>, jitter: f32, t: f32, metric: i32, mink: f32, seed: u32) -> Cells {
    let base = vec2<i32>(floor(x));
    var r = Cells(1e9, 1e9, vec2<i32>(0), vec2<i32>(0), vec2<f32>(0.0));
    for (var j = -2; j <= 2; j += 1) {
        for (var i = -2; i <= 2; i += 1) {
            let c = base + vec2<i32>(i, j);
            let q = cell_point(c, period, jitter, t, seed);
            let d = metric_length(q - x, metric, mink);
            if d < r.f1 {
                r.f2 = r.f1;
                r.id2 = r.id1;
                r.f1 = d;
                r.id1 = cell_wrap(c, period);
                r.p1 = q;
            } else if d < r.f2 {
                r.f2 = d;
                r.id2 = cell_wrap(c, period);
            }
        }
    }
    return r;
}

// Euclidean distance from x to the nearest cell wall (the bisector between the nearest point
// and its neighbours), in cells.
fn cell_edge(x: vec2<f32>, period: vec2<i32>, jitter: f32, t: f32, seed: u32) -> f32 {
    let near = cells(x, period, jitter, t, METRIC_EUCLIDEAN, 2.0, seed);
    let base = vec2<i32>(floor(near.p1));
    var edge = 1e9;
    for (var j = -2; j <= 2; j += 1) {
        for (var i = -2; i <= 2; i += 1) {
            let q = cell_point(base + vec2<i32>(i, j), period, jitter, t, seed);
            let d = q - near.p1;
            if dot(d, d) > 1e-8 {
                edge = min(edge, dot(0.5 * (q + near.p1) - x, normalize(d)));
            }
        }
    }
    return edge;
}

// 3D cells, tiling every `period` cells per axis (0: no tiling): one jittered point per cell,
// `jitter` per axis.
fn cell_point3(c: vec3<i32>, period: vec3<i32>, jitter: vec3<f32>, seed: u32) -> vec3<f32> {
    return vec3<f32>(c) + 0.5 + (hash33(wrap3(c, period), seed) - 0.5) * jitter;
}

struct Cells3 {
    f1: f32,
    f2: f32,
    id1: vec3<i32>,
    p1: vec3<f32>,
}

// Tiling in x and y.
fn cells3(x: vec3<f32>, period: vec2<i32>, jitter: vec3<f32>, metric: i32, mink: f32, seed: u32) -> Cells3 {
    return cells3p(x, vec3<i32>(period, 0), jitter, metric, mink, seed);
}

fn cells3p(x: vec3<f32>, period: vec3<i32>, jitter: vec3<f32>, metric: i32, mink: f32, seed: u32) -> Cells3 {
    let base = vec3<i32>(floor(x));
    var r = Cells3(1e9, 1e9, vec3<i32>(0), vec3<f32>(0.0));
    for (var k = -1; k <= 1; k += 1) {
        for (var j = -1; j <= 1; j += 1) {
            for (var i = -1; i <= 1; i += 1) {
                let c = base + vec3<i32>(i, j, k);
                let q = cell_point3(c, period, jitter, seed);
                let d = metric_length3(q - x, metric, mink);
                if d < r.f1 {
                    r.f2 = r.f1;
                    r.f1 = d;
                    r.id1 = wrap3(c, period);
                    r.p1 = q;
                } else if d < r.f2 {
                    r.f2 = d;
                }
            }
        }
    }
    return r;
}

// Distance from x to the nearest wall of its 3D cell, in cells; tiling in x and y.
fn cell_edge3(x: vec3<f32>, period: vec2<i32>, jitter: vec3<f32>, seed: u32) -> f32 {
    return cell_edge3p(x, vec3<i32>(period, 0), jitter, seed);
}

fn cell_edge3p(x: vec3<f32>, period: vec3<i32>, jitter: vec3<f32>, seed: u32) -> f32 {
    let near = cells3p(x, period, jitter, METRIC_EUCLIDEAN, 2.0, seed);
    let base = vec3<i32>(floor(near.p1));
    var edge = 1e9;
    for (var k = -1; k <= 1; k += 1) {
        for (var j = -1; j <= 1; j += 1) {
            for (var i = -1; i <= 1; i += 1) {
                let q = cell_point3(base + vec3<i32>(i, j, k), period, jitter, seed);
                let d = q - near.p1;
                if dot(d, d) > 1e-8 {
                    edge = min(edge, dot(0.5 * (q + near.p1) - x, normalize(d)));
                }
            }
        }
    }
    return edge;
}

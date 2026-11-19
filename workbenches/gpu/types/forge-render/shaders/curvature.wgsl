// Mesh curvature (see curvature.rs).
// `accumulate` adds one batch of disk samples from one sample point per texel to `acc` (x: sum
// of w * dn.d, y: sum of w * |d|^2, both scaled by the radius, for the current sample point;
// z: sum of each sample point's curvature times its weight, w: sum of the weights), `measure`
// bins the texels' |curvature| by octave fractions, `bound` picks the largest and
// `resolve` writes the normal map with the curvature in w.

struct Params {
    first: u32,
    samples: u32,
    radius: f32,
    // Bit 0: the sample point's first batch; bit 1: its last.
    flags: u32,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(4) var surface: texture_2d<f32>;
@group(0) @binding(5) var map_nrm: texture_2d<f32>;
@group(0) @binding(6) var<storage, read_write> acc: array<vec4<f32>>;
// BINS counts of |mean curvature * radius| on log2 bins, then the bound as f32 bits.
@group(0) @binding(7) var<storage, read_write> hist: array<atomic<u32>, 4097>;
@group(0) @binding(8) var out_normal: texture_storage_2d<rgba16float, write>;

// Bin 0 holds |H| below 2^LOG_LO, zero included; then BIN_STEPS bins per octave.
const BINS: u32 = 4096u;
const LOG_LO: f32 = -20.0;
const BIN_STEPS: f32 = 128.0;
// The share of texels let clip: none, bounded by the map's
// largest |H| (the reference curvature on the material ball spans 0.09..1, 99.8% inside 0.21..0.79).
const CLIP: f32 = 0.0;
// The tone curve on the bounded curvature: the reference map on the material ball reads gentle
// curvature ~3x and edges ~1.2x what a linear map gives, |c - 0.5| ~ |h|^0.82 / 2 (fitted
// inside the UV islands: MAE 0.0057 -> 0.0031; a narrow levels on the map, as smart materials'
// edge masks use, tells the exponent to +-0.03).
const TONE: f32 = 0.82;

// Hits kept per probe ray (LOCAL_MAX_HITS in Cycles' bevel.h).
const MAX_HITS: u32 = 4u;
const PI: f32 = 3.14159265;

// The cubic falloff's density over the disk (bevel.h svm_bevel_cubic_eval).
fn cubic(radius: f32, r: f32) -> f32 {
    if r >= radius {
        return 0.0;
    }
    let x = radius - r;
    return 10.0 * x * x * x / (PI * pow(radius, 5.0));
}

// The disk radius, over `radius`, where the cubic's CDF reaches `xi`: Newton steps from 0.25
// (bevel.h svm_bevel_cubic_quintic_root_find).
fn quintic_root(xi: f32) -> f32 {
    var x = 0.25;
    for (var i = 0; i < 10; i += 1) {
        let x2 = x * x;
        let x3 = x2 * x;
        let nx = 1.0 - x;
        let f = 10.0 * x2 - 20.0 * x3 + 15.0 * x2 * x2 - 4.0 * x2 * x3 - xi;
        let f_ = 20.0 * (x * nx) * (nx * nx);
        if abs(f) < 1e-6 || f_ == 0.0 {
            break;
        }
        x = saturate(x - f / f_);
    }
    return x;
}

@compute @workgroup_size(8, 8)
fn accumulate(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = textureDimensions(surface);
    if id.x >= size.x || id.y >= size.y {
        return;
    }
    let s = surface_at(textureLoad(surface, vec2<i32>(id.xy), 0));
    if s.weight == 0.0 {
        return;
    }
    let p = s.p;
    let n = s.n;
    let frame = basis(n);
    // The probe axes and the odds of each: the normal half the time, each tangent a quarter.
    var axes = array<vec3<f32>, 3>(n, frame[0], frame[1]);
    let pick = vec3<f32>(0.5, 0.25, 0.25);
    let radius = params.radius;
    let shift = jitter(id.xy);
    let i = id.y * size.x + id.x;
    var a = acc[i];
    if (params.flags & 1u) != 0u {
        a.x = 0.0;
        a.y = 0.0;
    }
    for (var k = params.first; k < params.first + params.samples; k += 1u) {
        let u = r2(k, shift);
        var axis = 0u;
        var x = u.x * 2.0;
        if u.x >= 0.75 {
            axis = 2u;
            x = (u.x - 0.75) * 4.0;
        } else if u.x >= 0.5 {
            axis = 1u;
            x = (u.x - 0.5) * 4.0;
        }
        // A disk across the picked axis, spanned by the other two; rays go down the axis
        // through a point on it.
        let disk_n = axes[axis];
        let disk_t = axes[(axis + 1u) % 3u];
        let disk_b = axes[(axis + 2u) % 3u];
        let disk_r = quintic_root(u.y) * radius;
        let h = sqrt(max(radius * radius - disk_r * disk_r, 0.0));
        if h <= 0.0 {
            continue;
        }
        let phi = 2.0 * PI * x;
        let o = p + disk_n * h + disk_r * (cos(phi) * disk_t + sin(phi) * disk_b);
        // The nearest hits in order; stepping past each skips the twin hit where two
        // triangles share an edge.
        var tmin = 0.0;
        for (var j = 0u; j < MAX_HITS; j += 1u) {
            let hit = trace(o, -disk_n, tmin, 2.0 * h, NONE, false);
            if hit.tri == NONE {
                break;
            }
            tmin = hit.t + 1e-4 * radius;
            let tri = tris[hit.tri];
            let tn = tri_normals[hit.tri];
            let b = hit.uv;
            let sn = (1.0 - b.x - b.y) * tn[0].xyz + b.x * tn[1].xyz + b.y * tn[2].xyz;
            if dot(sn, sn) < 1e-12 {
                continue;
            }
            let nq = normalize(sn);
            let ng = normalize(cross(tri.e1.xyz, tri.e2.xyz));
            let d = o - disk_n * hit.t - p;
            // Multiple importance sampling over the three axes with the power heuristic, and
            // the falloff at the true distance over the falloff that drew the sample (bevel.h).
            let pdfs = pick * abs(vec3<f32>(dot(axes[0], ng), dot(axes[1], ng), dot(axes[2], ng)));
            let w = pdfs[axis] / dot(pdfs, pdfs) * cubic(radius, length(d)) / cubic(radius, disk_r);
            a.x += w * dot(nq - n, d) / radius;
            a.y += w * dot(d, d) / (radius * radius);
        }
    }
    if (params.flags & 2u) != 0u && a.y > 0.0 {
        a.z += s.weight * a.x / a.y;
        a.w += s.weight;
    }
    acc[i] = a;
}

fn bin_of(v: f32) -> u32 {
    if v < exp2(LOG_LO) {
        return 0u;
    }
    return min(1u + u32((log2(v) - LOG_LO) * BIN_STEPS), BINS - 1u);
}

// Bins texels with at least one covered sample point.
@compute @workgroup_size(8, 8)
fn measure(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = textureDimensions(surface);
    if id.x >= size.x || id.y >= size.y {
        return;
    }
    let a = acc[id.y * size.x + id.x];
    if a.w >= 1.0 {
        atomicAdd(&hist[bin_of(abs(a.z / a.w))], 1u);
    }
}

// The |H| that CLIP of the texels exceed: the top of the bin where the count from the top
// passes it, or the largest bin when nearly all of the map is flat.
@compute @workgroup_size(1)
fn bound() {
    var total = 0u;
    var top = 0u;
    for (var b = 0u; b < BINS; b += 1u) {
        let c = atomicLoad(&hist[b]);
        total += c;
        if c > 0u {
            top = b;
        }
    }
    let clip = u32(f32(total) * CLIP);
    var pick = top;
    var above = 0u;
    for (var b = BINS - 1u; b > 0u; b -= 1u) {
        above += atomicLoad(&hist[b]);
        if above > clip {
            pick = b;
            break;
        }
    }
    let value = select(0.0, exp2(LOG_LO + f32(pick) / BIN_STEPS), pick > 0u);
    atomicStore(&hist[BINS], bitcast<u32>(value));
}

@compute @workgroup_size(8, 8)
fn resolve(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = textureDimensions(surface);
    if id.x >= size.x || id.y >= size.y {
        return;
    }
    let texel = vec2<i32>(id.xy);
    let a = acc[id.y * size.x + id.x];
    let bound = bitcast<f32>(atomicLoad(&hist[BINS]));
    // Flat stays 0.5; the most curved texel reads 0 or 1.
    var c = 0.5;
    if a.w > 0.0 && bound > 0.0 {
        let h = clamp(a.z / a.w / bound, -1.0, 1.0);
        c = 0.5 + 0.5 * sign(h) * pow(abs(h), TONE);
    }
    textureStore(out_normal, texel, vec4<f32>(textureLoad(map_nrm, texel, 0).xyz, c));
}

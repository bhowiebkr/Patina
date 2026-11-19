// Image-based lighting from an equirectangular panorama (u: longitude, 0.5 looks down -Z; v: zenith
// to nadir; Y up), as Bevy's environment map light reads it (split sum, Karis 2013):
// - load: the f32 panorama into mip 0 of a half-float copy; down: its box-filtered mip chain;
// - radiance: GGX-prefiltered radiance, one mip per perceptual roughness level/(levels-1), by
//   importance sampling with filtered importance sampling (GPU Gems 3, ch. 20);
// - irradiance: (1/pi) * the cosine-weighted integral of radiance over the hemisphere, summed
//   over every texel of a small source mip.

struct Params {
    // Output size, and the radiance level and level count.
    size: vec2<u32>,
    level: u32,
    levels: u32,
    // Source mip 0 size, the source mip the irradiance sums, samples per texel.
    source_size: vec2<u32>,
    mip: u32,
    samples: u32,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(2) var source: texture_2d<f32>;
@group(0) @binding(3) var samp: sampler;
@group(0) @binding(4) var dst: texture_storage_2d<rgba16float, write>;

const PI: f32 = 3.14159265;
const TAU: f32 = 6.28318531;

fn dir_of(uv: vec2<f32>) -> vec3<f32> {
    let phi = (uv.x - 0.5) * TAU;
    let theta = uv.y * PI;
    return vec3<f32>(sin(theta) * sin(phi), cos(theta), -sin(theta) * cos(phi));
}

fn uv_of(d: vec3<f32>) -> vec2<f32> {
    return vec2<f32>(fract(atan2(d.x, -d.z) / TAU + 0.5), acos(clamp(d.y, -1.0, 1.0)) / PI);
}

fn texel_dir(gid: vec2<u32>) -> vec3<f32> {
    return dir_of((vec2<f32>(gid) + 0.5) / vec2<f32>(params.size));
}

@group(0) @binding(1) var raw: texture_2d<f32>;

@compute @workgroup_size(8, 8)
fn load(@builtin(global_invocation_id) gid: vec3<u32>) {
    if any(gid.xy >= params.size) {
        return;
    }
    let c = textureLoad(raw, gid.xy, 0);
    // NaN and infinities (and negatives) from the file would spread through every filter.
    let ok = all(c.rgb == c.rgb) && all(abs(c.rgb) < vec3<f32>(65000.0));
    textureStore(dst, gid.xy, vec4<f32>(select(vec3<f32>(0.0), max(c.rgb, vec3<f32>(0.0)), ok), 1.0));
}

@compute @workgroup_size(8, 8)
fn down(@builtin(global_invocation_id) gid: vec3<u32>) {
    if any(gid.xy >= params.size) {
        return;
    }
    let last = textureDimensions(source) - 1u;
    let p = gid.xy * 2u;
    let c = textureLoad(source, min(p, last), 0) + textureLoad(source, min(p + vec2<u32>(1u, 0u), last), 0)
        + textureLoad(source, min(p + vec2<u32>(0u, 1u), last), 0) + textureLoad(source, min(p + 1u, last), 0);
    textureStore(dst, gid.xy, c * 0.25);
}

fn hammersley(i: u32, n: u32) -> vec2<f32> {
    return vec2<f32>(f32(i) / f32(n), f32(reverseBits(i)) * 2.3283064365386963e-10);
}

@compute @workgroup_size(8, 8)
fn radiance(@builtin(global_invocation_id) gid: vec3<u32>) {
    if any(gid.xy >= params.size) {
        return;
    }
    let n = texel_dir(gid.xy);
    let src = vec2<f32>(params.source_size);
    if params.level == 0u {
        let lod = max(log2(src.x / f32(params.size.x)), 0.0);
        textureStore(dst, gid.xy, vec4<f32>(textureSampleLevel(source, samp, uv_of(n), lod).rgb, 1.0));
        return;
    }
    let pr = f32(params.level) / f32(max(params.levels - 1u, 1u));
    let a = pr * pr;
    let a2 = a * a;
    let up = select(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 1.0, 0.0), abs(n.y) < 0.999);
    let tx = normalize(cross(up, n));
    let ty = cross(n, tx);
    // Solid angle of a source texel at the equator; the sine of the polar angle scales it.
    let texel = (TAU / src.x) * (PI / src.y);
    var sum = vec3<f32>(0.0);
    var weight = 0.0;
    for (var i = 0u; i < params.samples; i++) {
        let xi = hammersley(i, params.samples);
        let phi = TAU * xi.x;
        let ct = sqrt((1.0 - xi.y) / (1.0 + (a2 - 1.0) * xi.y));
        let st = sqrt(max(1.0 - ct * ct, 0.0));
        let h = tx * (st * cos(phi)) + ty * (st * sin(phi)) + n * ct;
        // N = V = R: the reflection of the view about h.
        let l = 2.0 * dot(n, h) * h - n;
        let nl = dot(n, l);
        if nl > 0.0 {
            let dd = ct * ct * (a2 - 1.0) + 1.0;
            let pdf = a2 / (PI * dd * dd) * 0.25;
            let omega_s = 1.0 / (f32(params.samples) * pdf + 1e-6);
            let omega_p = texel * max(sqrt(max(1.0 - l.y * l.y, 0.0)), 1e-3);
            let lod = max(0.5 * log2(omega_s / omega_p) + 1.0, 0.0);
            sum += textureSampleLevel(source, samp, uv_of(l), lod).rgb * nl;
            weight += nl;
        }
    }
    textureStore(dst, gid.xy, vec4<f32>(sum / max(weight, 1e-6), 1.0));
}

@compute @workgroup_size(8, 8)
fn irradiance(@builtin(global_invocation_id) gid: vec3<u32>) {
    if any(gid.xy >= params.size) {
        return;
    }
    let n = texel_dir(gid.xy);
    let dims = textureDimensions(source, params.mip);
    var sum = vec3<f32>(0.0);
    for (var y = 0u; y < dims.y; y++) {
        let v = (f32(y) + 0.5) / f32(dims.y);
        let sine = sin(v * PI);
        for (var x = 0u; x < dims.x; x++) {
            let l = dir_of(vec2<f32>((f32(x) + 0.5) / f32(dims.x), v));
            let c = dot(n, l);
            if c > 0.0 {
                sum += textureLoad(source, vec2<u32>(x, y), i32(params.mip)).rgb * (c * sine);
            }
        }
    }
    // Texel solid angle (2pi/W)(pi/H) sin(theta), over pi.
    let scale = (TAU / f32(dims.x)) * (PI / f32(dims.y)) / PI;
    textureStore(dst, gid.xy, vec4<f32>(sum * scale, 1.0));
}

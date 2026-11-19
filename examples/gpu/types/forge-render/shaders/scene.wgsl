// Forward PBR for one asset: GGX sun (soft when it has an angular size) + sky dome ambient or an
// environment panorama (Bevy's environment map light), ACES or a plain clip. Hot-reloaded while
// the app runs.

struct Frame {
    view_proj: mat4x4<f32>,
    eye: vec4<f32>,
    // Direction to the sun (xyz) and its intensity (w).
    sun: vec4<f32>,
    sky: vec4<f32>,
    ground: vec4<f32>,
    // x: exposure; y: 1 = plain clip (Blender's Standard view), 0 = ACES; z: sine of the sun's
    // angular radius (0 = a point sun); w: 0 lit, else one channel unlit (1 base colour,
    // 2 roughness, 3 metallic, 4 height, 5 ambient occlusion, 6 normal).
    misc: vec4<f32>,
    // x: environment strength (0 = the sky and ground instead); y: its turn about Y, in turns;
    // z: the radiance's last mip.
    env: vec4<f32>,
}

struct Part {
    model: mat4x4<f32>,
    // Linear base colour.
    color: vec4<f32>,
    // metallic, roughness, use material planes, visible.
    pbr: vec4<f32>,
    highlight: vec4<f32>,
}

@group(0) @binding(0) var<uniform> frame: Frame;
@group(1) @binding(0) var<storage, read> parts: array<Part>;
// Smart-material output: sRGB base colour + opacity; roughness, metallic, height, ao;
// tangent-space normal (0..1, OpenGL, +Y up).
@group(1) @binding(1) var plane0: texture_2d<f32>;
@group(1) @binding(2) var plane1: texture_2d<f32>;
@group(1) @binding(3) var samp: sampler;
@group(1) @binding(4) var plane2: texture_2d<f32>;
// Equirectangular panoramas (u: longitude, 0.5 looks down -Z; v: zenith to nadir): GGX-prefiltered
// radiance, one perceptual roughness level/(levels-1) per mip, and diffuse irradiance / pi.
@group(2) @binding(0) var env_radiance: texture_2d<f32>;
@group(2) @binding(1) var env_irradiance: texture_2d<f32>;
@group(2) @binding(2) var env_samp: sampler;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) @interpolate(flat) part: u32,
}

@vertex
fn vs(
    @location(0) pos: vec3<f32>,
    @location(1) nrm: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(4) part: u32,
) -> VsOut {
    let p = parts[part];
    let world = p.model * vec4<f32>(pos, 1.0);
    var o: VsOut;
    o.clip = frame.view_proj * world;
    if p.pbr.w < 0.5 {
        // Hidden part: behind the near plane, so the whole triangle clips away.
        o.clip = vec4<f32>(0.0, 0.0, -2.0, 1.0);
    }
    o.world = world.xyz;
    o.normal = (p.model * vec4<f32>(nrm, 0.0)).xyz;
    o.uv = uv;
    o.part = part;
    return o;
}

const PI: f32 = 3.14159265;
const TAU: f32 = 6.28318531;

fn env_uv(d: vec3<f32>) -> vec2<f32> {
    let a = frame.env.y * TAU;
    let yaw = vec3<f32>(d.x * cos(a) - d.z * sin(a), d.y, d.x * sin(a) + d.z * cos(a));
    let b = frame.env.w * TAU;
    let r = vec3<f32>(yaw.x, yaw.y * cos(b) - yaw.z * sin(b), yaw.y * sin(b) + yaw.z * cos(b));
    return vec2<f32>(fract(atan2(r.x, -r.z) / TAU + 0.5), acos(clamp(r.y, -1.0, 1.0)) / PI);
}

// Bevy's F_AB: the split-sum DFG terms, Karis's analytic fit (Mobile, 2014).
fn env_brdf(perceptual: f32, ndv: f32) -> vec2<f32> {
    let c0 = vec4<f32>(-1.0, -0.0275, -0.572, 0.022);
    let c1 = vec4<f32>(1.0, 0.0425, 1.04, -0.04);
    let r = perceptual * c0 + c1;
    let a004 = min(r.x * r.x, exp2(-9.28 * ndv)) * r.x + r.y;
    return vec2<f32>(-1.04, 1.04) * a004 + r.zw;
}

// Bevy's environment light with its multiple-scattering compensation (Fdez-Aguera 2019) and
// Lagarde's specular occlusion from the ambient occlusion.
fn environment(n: vec3<f32>, v: vec3<f32>, base: vec3<f32>, metal: f32, rough: f32, ao: f32) -> vec3<f32> {
    let ndv = max(dot(n, v), 1e-4);
    let r = reflect(-v, n);
    let f0 = mix(vec3<f32>(0.04), base, metal);
    let alpha = rough * rough;
    let irradiance = textureSampleLevel(env_irradiance, env_samp, env_uv(n), 0.0).rgb;
    let radiance = textureSampleLevel(env_radiance, env_samp, env_uv(r), rough * frame.env.z).rgb;
    let fab = env_brdf(rough, ndv);
    let ess = fab.x + fab.y;
    // F_AB already integrates the angular Fresnel term. Applying Schlick a second time
    // over-brightens grazing normals (the white rim seen when turning the material ball).
    let floor = clamp(dot(f0, vec3<f32>(50.0 * 0.33)), 0.0, 1.0);
    let fss_ess = (f0 * fab.x + fab.y) * floor;
    let ems = 1.0 - ess;
    let favg = f0 + (1.0 - f0) / 21.0;
    let fms = fss_ess * favg / (1.0 - ems * favg);
    let fms_ems = fms * ems;
    let edss = 1.0 - (fss_ess + fms_ems);
    let kd = base * (1.0 - metal) * edss;
    let diffuse = (fms_ems + kd) * irradiance * ao;
    let occlusion = clamp(pow(ndv + ao, exp2(-16.0 * alpha - 1.0)) - 1.0 + ao, 0.0, 1.0);
    return (diffuse + fss_ess * radiance * occlusion) * frame.env.x;
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    return select(pow((c + 0.055) / 1.055, vec3<f32>(2.4)), c / 12.92, c <= vec3<f32>(0.04045));
}

// Tangent-space normal onto the surface, with a frame from screen derivatives (no tangents).
fn perturb(n: vec3<f32>, world: vec3<f32>, uv: vec2<f32>, t: vec3<f32>) -> vec3<f32> {
    let dp1 = dpdx(world);
    let dp2 = dpdy(world);
    let du1 = dpdx(uv);
    let du2 = dpdy(uv);
    let p2 = cross(dp2, n);
    let p1 = cross(n, dp1);
    let tan = p2 * du1.x + p1 * du2.x;
    let bit = p2 * du1.y + p1 * du2.y;
    let scale = inverseSqrt(max(max(dot(tan, tan), dot(bit, bit)), 1e-20));
    // UV v runs down the texture, so +Y (up in the image) is -bitangent.
    let m = normalize(tan * scale * t.x - bit * scale * t.y + n * t.z);
    return select(n, m, all(m == m));
}

fn aces(x: vec3<f32>) -> vec3<f32> {
    return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), vec3<f32>(0.0), vec3<f32>(1.0));
}

fn shade(in: VsOut, front: bool) -> vec4<f32> {
    let p = parts[in.part];
    let m0 = textureSample(plane0, samp, in.uv);
    let m1 = textureSample(plane1, samp, in.uv);
    let m2 = textureSample(plane2, samp, in.uv).xyz * 2.0 - 1.0;
    let maps = p.pbr.z > 0.5;
    let base = select(p.color.rgb, srgb_to_linear(clamp(m0.rgb, vec3<f32>(0.0), vec3<f32>(1.0))), maps);
    let rough = clamp(select(p.pbr.y, m1.r, maps), 0.045, 1.0);
    let metal = clamp(select(p.pbr.x, m1.g, maps), 0.0, 1.0);
    let occlusion = select(1.0, clamp(m1.a, 0.0, 1.0), maps);

    var n = normalize(in.normal);
    if !front {
        n = -n;
    }
    let bumped = perturb(n, in.world, in.uv, m2);
    if maps {
        n = bumped;
    }
    // Channel views show stored values: grey data is linearised so the sRGB target shows it as is.
    let gray = select(1.0, m1.b, maps);
    switch u32(frame.misc.w + 0.5) {
        case 1u: {
            return vec4<f32>(base, 1.0);
        }
        case 2u: {
            return vec4<f32>(srgb_to_linear(vec3<f32>(rough)), 1.0);
        }
        case 3u: {
            return vec4<f32>(srgb_to_linear(vec3<f32>(metal)), 1.0);
        }
        case 4u: {
            return vec4<f32>(srgb_to_linear(vec3<f32>(clamp(gray, 0.0, 1.0))), 1.0);
        }
        case 5u: {
            return vec4<f32>(srgb_to_linear(vec3<f32>(occlusion)), 1.0);
        }
        case 6u: {
            let t = select(vec3<f32>(0.5, 0.5, 1.0), m2 * 0.5 + 0.5, maps);
            return vec4<f32>(srgb_to_linear(clamp(t, vec3<f32>(0.0), vec3<f32>(1.0))), 1.0);
        }
        default: {}
    }
    let v = normalize(frame.eye.xyz - in.world);
    let l = normalize(frame.sun.xyz);
    let h = normalize(v + l);
    let nl = dot(n, l);
    let ndl = max(nl, 0.0);
    // A sun disc of angular radius asin(sa) still lights a surface turned up to that far past its
    // horizon: a smooth wrap below nl = sa, exact above it.
    let sa = frame.misc.z;
    let wrap = max(nl + sa, 0.0) * max(nl + sa, 0.0) / max(4.0 * sa, 1e-6);
    let soft = select(ndl, wrap, nl < sa);
    let ndv = max(dot(n, v), 1e-4);
    let ndh = max(dot(n, h), 0.0);
    let vdh = max(dot(v, h), 0.0);

    // The disc widens the highlight: its tangent half-width added to the GGX alpha.
    let alpha = min(rough * rough + 0.5 * sa / sqrt(max(1.0 - sa * sa, 1e-4)), 1.0);
    let a2 = alpha * alpha;
    let dd = ndh * ndh * (a2 - 1.0) + 1.0;
    let d = a2 / (PI * dd * dd);
    let k = (rough + 1.0) * (rough + 1.0) / 8.0;
    let g = ndv / (ndv * (1.0 - k) + k) * ndl / (ndl * (1.0 - k) + k);
    let f0 = mix(vec3<f32>(0.04), base, metal);
    let f = f0 + (1.0 - f0) * pow(1.0 - vdh, 5.0);
    let spec = d * g * f / (4.0 * ndv * max(ndl, 1e-4));
    let diffuse = (1.0 - f) * (1.0 - metal) * base / PI;
    let direct = (diffuse * soft + spec * ndl) * frame.sun.w;

    let hemi = mix(frame.ground.rgb, frame.sky.rgb, n.y * 0.5 + 0.5);
    let r = reflect(-v, n);
    // Sharp reflections see the horizon; rough ones a blur of the whole dome.
    let up = mix(smoothstep(-0.1, 0.1, r.y), r.y * 0.5 + 0.5, rough);
    let env = mix(frame.ground.rgb, frame.sky.rgb, up);
    let fr = f0 + (max(vec3<f32>(1.0 - rough), f0) - f0) * pow(1.0 - ndv, 5.0);
    var ambient = (hemi * base * (1.0 - metal) + env * fr * (1.0 - 0.6 * rough)) * occlusion;
    if frame.env.x > 0.0 {
        ambient = environment(n, v, base, metal, rough, occlusion);
    }

    let c = (direct + ambient) * frame.misc.x;
    return vec4<f32>(select(aces(c), clamp(c, vec3<f32>(0.0), vec3<f32>(1.0)), frame.misc.y > 0.5), 1.0);
}

@fragment
fn fs(in: VsOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    let shaded = shade(in, front);
    let highlight = parts[in.part].highlight;
    if highlight.a <= 0.0 {
        return shaded;
    }
    return vec4<f32>(mix(shaded.rgb, highlight.rgb, highlight.a), shaded.a);
}

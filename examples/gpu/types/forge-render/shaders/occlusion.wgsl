// Ray-traced ambient occlusion and thickness, and the distance to the nearest other part
// (see occlusion.rs). `accumulate` adds one batch of
// cosine-weighted rays from one sample point per texel to `acc` (x: occlusion, y: inward
// distance, z: rays, each scaled by the sample's weight; w: covered samples), `near_part` adds
// that sample point's closeness to `near_sums`, `measure` finds
// the map's thickness range (only its max is used) and `resolve` writes the extras map.

struct Params {
    first: u32,
    rays: u32,
    min_dist: f32,
    ao_max: f32,
    thick_max: f32,
    near_max: f32,
    _p1: f32,
    _p2: f32,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(4) var surface: texture_2d<f32>;
@group(0) @binding(5) var map_part: texture_2d<f32>;
@group(0) @binding(6) var<storage, read_write> acc: array<vec4<f32>>;
// The lowest and highest mean inward distance, as f32 bits (they are never negative).
@group(0) @binding(7) var<storage, read_write> range_bits: array<atomic<u32>, 2>;
@group(0) @binding(8) var out_extras: texture_storage_2d<rgba16float, write>;
// x: closeness to another part (1 touching, 0 at `near_max`), y: weight, each scaled by the
// sample's weight. Summing closeness keeps a texel with no other part in reach at exactly 1.
@group(0) @binding(10) var<storage, read_write> near_sums: array<vec2<f32>>;

@compute @workgroup_size(8, 8)
fn near_part(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = textureDimensions(surface);
    if id.x >= size.x || id.y >= size.y {
        return;
    }
    let s = surface_at(textureLoad(surface, vec2<i32>(id.xy), 0));
    if s.weight == 0.0 {
        return;
    }
    let d = distance_to_other_part(s.p, u32(tris[s.slot].v0.w), params.near_max);
    near_sums[id.y * size.x + id.x] += s.weight * vec2<f32>(1.0 - d / params.near_max, 1.0);
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
    let frame = basis(s.n);
    let out = terminator(s, false);
    let into = terminator(s, true);
    let shift = jitter(id.xy);
    let i = id.y * size.x + id.x;
    var a = acc[i];
    for (var k = params.first; k < params.first + params.rays; k += 1u) {
        // Linear attenuation: an occluder at the surface occludes fully, one at the maximum
        // distance not at all; a miss returns the maximum distance.
        let ao = trace_group(out, frame * cosine_dir(k, shift), params.min_dist, params.ao_max,
            s.slot, false, u32(tris[s.slot].e1.w));
        // Inward rays stop only where they leave the solid ("Ignore Backface").
        let back = -(frame * cosine_dir(k, shift.yx));
        let thick = trace(into, back, params.min_dist, params.thick_max, s.slot, true);
        a += vec4<f32>(s.weight * vec3<f32>(1.0 - ao.t / params.ao_max, thick.t, 1.0),
            select(0.0, 1.0, s.weight == 1.0));
    }
    acc[i] = a;
}

@compute @workgroup_size(8, 8)
fn measure(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = textureDimensions(surface);
    if id.x >= size.x || id.y >= size.y {
        return;
    }
    let a = acc[id.y * size.x + id.x];
    if a.w <= 0.0 {
        return;
    }
    let d = bitcast<u32>(a.y / a.z);
    atomicMin(&range_bits[0], d);
    atomicMax(&range_bits[1], d);
}

@compute @workgroup_size(8, 8)
fn resolve(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = textureDimensions(surface);
    if id.x >= size.x || id.y >= size.y {
        return;
    }
    let texel = vec2<i32>(id.xy);
    let part = textureLoad(map_part, texel, 0).r;
    let a = acc[id.y * size.x + id.x];
    let n = near_sums[id.y * size.x + id.x];
    let near = saturate(1.0 - select(0.0, n.x / n.y, n.y > 0.0));
    if a.z <= 0.0 {
        textureStore(out_extras, texel, vec4<f32>(1.0, 1.0, part, near));
        return;
    }
    // "Relative to Min/Max": the thickest texel reads 1, zero distance 0.
    let hi = bitcast<f32>(atomicLoad(&range_bits[1]));
    let d = a.y / a.z;
    let thickness = select(1.0, saturate(d / hi), hi > 0.0);
    textureStore(out_extras, texel, vec4<f32>(1.0 - a.x / a.z, thickness, part, near));
}

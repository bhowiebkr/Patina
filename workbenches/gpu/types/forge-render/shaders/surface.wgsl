// The surface point under each texel, as the BVH triangle it lies on and its barycentrics, for
// the occlusion and curvature bakers (see surface.rs). `vs`/`fs` rasterise the mesh in UV space
// shifted to one of four sample points in each texel; `dilate` fills empty texels from the
// nearest covered texel within RADIUS (coverage 0.5 marks a fill).

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) bary: vec2<f32>,
    @location(1) @interpolate(flat) slot: u32,
}

@vertex
fn vs(
    @location(0) uv: vec2<f32>,
    @location(1) slot: u32,
    @location(2) corner: u32,
    @location(3) shift: vec2<f32>,
) -> VsOut {
    var o: VsOut;
    o.clip = vec4<f32>(uv.x * 2.0 - 1.0 + shift.x, 1.0 - uv.y * 2.0 + shift.y, 0.5, 1.0);
    // Barycentrics on the triangle's e1 and e2: corner 0 is (0, 0).
    o.bary = vec2<f32>(f32(corner == 1u), f32(corner == 2u));
    o.slot = slot;
    return o;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    return vec4<f32>(f32(in.slot), in.bary, 1.0);
}

@group(0) @binding(0) var raw: texture_2d<f32>;
@group(0) @binding(1) var dilated: texture_storage_2d<rgba32float, write>;

const RADIUS: i32 = 4;

@compute @workgroup_size(8, 8)
fn dilate(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = vec2<i32>(textureDimensions(raw));
    let p = vec2<i32>(id.xy);
    if p.x >= size.x || p.y >= size.y {
        return;
    }
    var s = textureLoad(raw, p, 0);
    if s.w < 0.5 {
        var best = 1e9;
        for (var dy = -RADIUS; dy <= RADIUS; dy += 1) {
            for (var dx = -RADIUS; dx <= RADIUS; dx += 1) {
                let c = textureLoad(raw, clamp(p + vec2<i32>(dx, dy), vec2<i32>(0), size - 1), 0);
                let d = f32(dx * dx + dy * dy);
                if c.w > 0.5 && d < best {
                    best = d;
                    s = vec4<f32>(c.xyz, 0.5);
                }
            }
        }
    }
    textureStore(dilated, p, s);
}

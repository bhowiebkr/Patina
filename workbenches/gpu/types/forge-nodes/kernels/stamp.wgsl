// One brush dab into a Paint Mask node's mask (R32Float, in the mesh maps' UV layout). A texel
// is under the dab when its rest-pose surface point lies within the brush radius of the picked
// point and faces the same way; the radius is in screen pixels, scaled to model units at the
// picked distance. Fill sets every texel of the picked part. With symmetry on, the dab is also
// stamped mirrored across a model-axis plane.

struct Brush {
    // Radius (px), model units per pixel at unit distance, strength, hardness.
    shape: vec4<f32>,
    // Tool (0 brush, 1 eraser, 2 fill), map size, mirror axis (0 none, 1 x, 2 y, 3 z).
    mode: vec4<u32>,
    // Mirror plane coordinate (x).
    mirror: vec4<f32>,
}

@group(0) @binding(0) var<uniform> brush: Brush;
// Rest-pose point under the pointer (xyz) and its distance from the eye (w; -1 on a miss).
@group(0) @binding(1) var hit_pos: texture_2d<f32>;
// Rest-pose normal (xyz) and part index (w).
@group(0) @binding(2) var hit_nrm: texture_2d<f32>;
@group(0) @binding(3) var map_pos: texture_2d<f32>;
@group(0) @binding(4) var map_nrm: texture_2d<f32>;
@group(0) @binding(5) var map_extras: texture_2d<f32>;
@group(0) @binding(6) var mask: texture_storage_2d<r32float, read_write>;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x >= brush.mode.y || id.y >= brush.mode.y {
        return;
    }
    let hit = textureLoad(hit_pos, vec2<i32>(0), 0);
    let p = vec2<i32>(id.xy);
    let pos = textureLoad(map_pos, p, 0);
    if hit.w < 0.0 || pos.w <= 0.0 {
        return;
    }
    let hn = textureLoad(hit_nrm, vec2<i32>(0), 0);
    let strength = brush.shape.z;
    if brush.mode.x == 2u {
        if abs(textureLoad(map_extras, p, 0).b - hn.w) < 0.5 {
            textureStore(mask, p, vec4<f32>(strength, 0.0, 0.0, 1.0));
        }
        return;
    }
    let r = brush.shape.x * brush.shape.y * hit.w;
    let n = textureLoad(map_nrm, p, 0).xyz;
    var d = 1e30;
    if dot(n, hn.xyz) >= 0.0 {
        d = distance(pos.xyz, hit.xyz);
    }
    if brush.mode.z > 0u {
        let axis = brush.mode.z - 1u;
        var mp = hit.xyz;
        var mn = hn.xyz;
        mp[axis] = 2.0 * brush.mirror.x - mp[axis];
        mn[axis] = -mn[axis];
        if dot(n, mn) >= 0.0 {
            d = min(d, distance(pos.xyz, mp));
        }
    }
    if d > r {
        return;
    }
    let f = strength * (1.0 - smoothstep(min(brush.shape.w, 0.99), 1.0, d / r));
    let old = textureLoad(mask, p).r;
    let v = select(max(old, f), min(old, 1.0 - f), brush.mode.x == 1u);
    textureStore(mask, p, vec4<f32>(v, 0.0, 0.0, 1.0));
}

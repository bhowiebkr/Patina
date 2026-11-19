// Fills empty texels from the nearest covered texel within RADIUS (coverage 0.5 marks a fill).

@group(0) @binding(0) var src_pos: texture_2d<f32>;
@group(0) @binding(1) var src_nrm: texture_2d<f32>;
@group(0) @binding(2) var dst_pos: texture_storage_2d<rgba32float, write>;
@group(0) @binding(3) var dst_nrm: texture_storage_2d<rgba16float, write>;
@group(0) @binding(4) var src_part: texture_2d<f32>;
@group(0) @binding(5) var dst_part: texture_storage_2d<r32float, write>;

const RADIUS: i32 = 4;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = vec2<i32>(textureDimensions(src_pos));
    let p = vec2<i32>(id.xy);
    if p.x >= size.x || p.y >= size.y {
        return;
    }
    var pos = textureLoad(src_pos, p, 0);
    var nrm = textureLoad(src_nrm, p, 0);
    var part = textureLoad(src_part, p, 0);
    if pos.w < 0.5 {
        var best = 1e9;
        for (var dy = -RADIUS; dy <= RADIUS; dy += 1) {
            for (var dx = -RADIUS; dx <= RADIUS; dx += 1) {
                let q = clamp(p + vec2<i32>(dx, dy), vec2<i32>(0), size - 1);
                let c = textureLoad(src_pos, q, 0);
                let d = f32(dx * dx + dy * dy);
                if c.w > 0.5 && d < best {
                    best = d;
                    pos = vec4<f32>(c.xyz, 0.5);
                    nrm = textureLoad(src_nrm, q, 0);
                    part = textureLoad(src_part, q, 0);
                }
            }
        }
    }
    textureStore(dst_pos, p, pos);
    textureStore(dst_nrm, p, nrm);
    textureStore(dst_part, p, part);
}

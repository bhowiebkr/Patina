@group(0) @binding(0) var hit_normal: texture_2d<f32>;
@group(0) @binding(1) var extras: texture_2d<f32>;
@group(0) @binding(2) var before: texture_2d<f32>;
@group(0) @binding(3) var mask: texture_storage_2d<r32float, read_write>;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) index: vec3<u32>) {
    if any(index.xy >= textureDimensions(mask)) { return; }
    let pixel = vec2<i32>(index.xy);
    let picked = textureLoad(hit_normal, vec2<i32>(0), 0).w;
    let part = textureLoad(extras, pixel, 0).b;
    if abs(part - picked) >= 0.5 {
        textureStore(mask, pixel, textureLoad(before, pixel, 0));
    }
}

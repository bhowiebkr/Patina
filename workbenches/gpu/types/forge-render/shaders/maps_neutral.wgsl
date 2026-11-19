@group(0) @binding(0) var normal: texture_2d<f32>;
@group(0) @binding(1) var part: texture_2d<f32>;
@group(0) @binding(2) var output_normal: texture_storage_2d<rgba16float, write>;
@group(0) @binding(3) var output_extras: texture_storage_2d<rgba16float, write>;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = textureDimensions(normal);
    if id.x >= size.x || id.y >= size.y {
        return;
    }
    let texel = vec2<i32>(id.xy);
    textureStore(output_normal, texel, vec4<f32>(textureLoad(normal, texel, 0).xyz, 0.5));
    textureStore(output_extras, texel, vec4<f32>(1.0, 1.0, textureLoad(part, texel, 0).r, 1.0));
}

import package::shade::shade;

@group(0) @binding(0) var image: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3u) {
    let size = textureDimensions(image);
    if id.x >= size.x || id.y >= size.y {
        return;
    }
    let t = f32(id.x) / f32(max(size.x - 1u, 1u));
    textureStore(image, id.xy, vec4f(shade(t), 1.0));
}

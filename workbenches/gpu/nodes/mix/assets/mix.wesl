import patina_gpu::image::inside;

// How much of b is in the mix, from 0 to 1. The node sets it when it makes the pipeline.
override amount: f32 = 0.5;

@group(0) @binding(0) var a: texture_2d<f32>;
@group(0) @binding(1) var b: texture_2d<f32>;
@group(0) @binding(2) var mixed: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3u) {
    let size = textureDimensions(mixed);
    if !inside(id, size) {
        return;
    }
    // The image b can have a different size, so the shader scales it to the size of a.
    let at = id.xy * textureDimensions(b) / size;
    textureStore(mixed, id.xy, mix(textureLoad(a, id.xy, 0), textureLoad(b, at, 0), amount));
}

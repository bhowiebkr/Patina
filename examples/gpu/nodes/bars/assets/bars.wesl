import patina_gpu::image::inside;

// The brightness of the bars, from 0 to 1. The node sets it when it makes the pipeline.
override level: f32 = 0.75;

@group(0) @binding(0) var image: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3u) {
    let size = textureDimensions(image);
    if !inside(id, size) {
        return;
    }
    var color: vec3f;
    if id.y * 4u >= size.y * 3u {
        // A gray ramp from black to white under the bars.
        color = vec3f(f32(id.x) / f32(max(size.x - 1u, 1u)));
    } else {
        // White, yellow, cyan, green, magenta, red and blue, as in a test signal.
        let bar = min(id.x * 7u / size.x, 6u);
        color = vec3f(f32((bar & 2u) == 0u), f32(bar < 4u), f32((bar & 1u) == 0u)) * level;
    }
    textureStore(image, id.xy, vec4f(color, 1.0));
}

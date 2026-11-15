// The shared image functions for node shaders (spec 10.3). Import them with
// `import patina_gpu::image::luma;`.

// The luma of a linear RGB color, with the Rec. 709 weights.
public fn luma(color: vec3f) -> f32 {
    return dot(color, vec3f(0.2126, 0.7152, 0.0722));
}

// True when the invocation is inside the image. A shader with 8 x 8 workgroups calls this first.
public fn inside(id: vec3u, size: vec2u) -> bool {
    return id.x < size.x && id.y < size.y;
}

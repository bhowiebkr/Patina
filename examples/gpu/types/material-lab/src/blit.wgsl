@group(0) @binding(0) var source: texture_2d<f32>;
struct Vertex { @builtin(position) position: vec4f, @location(0) uv: vec2f }
@vertex fn vs(@builtin(vertex_index) index: u32) -> Vertex {
    let uv = vec2f(f32((index << 1u) & 2u), f32(index & 2u));
    return Vertex(vec4f(uv * 2.0 - 1.0, 0.0, 1.0), vec2f(uv.x, 1.0 - uv.y));
}
fn sample(uv: vec2f) -> vec3f {
    let size = vec2i(textureDimensions(source));
    return textureLoad(source, clamp(vec2i(uv * vec2f(size)), vec2i(0), size - 1), 0).rgb;
}
@fragment fn linear(v: Vertex) -> @location(0) vec4f { return vec4f(sample(v.uv), 1.0); }
@fragment fn encoded(v: Vertex) -> @location(0) vec4f {
    let color = max(sample(v.uv), vec3f(0.0));
    return vec4f(select(1.055 * pow(color, vec3f(1.0 / 2.4)) - 0.055, color * 12.92, color <= vec3f(0.0031308)), 1.0);
}

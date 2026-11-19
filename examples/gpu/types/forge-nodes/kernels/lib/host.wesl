// Host images (Bitmap, Text, SVG): the CPU raster bound at binding 6 in place of scratch,
// top row first. Reads outside it clamp to the edge.

fn has_host() -> bool {
    return (info.domain.w & 1u) != 0u;
}

fn host_size() -> vec2<i32> {
    return vec2<i32>(textureDimensions(scratch_in));
}

fn host_load(q: vec2<i32>) -> vec4<f32> {
    if !has_host() {
        return vec4<f32>(0.0);
    }
    return textureLoad(scratch_in, clamp(q, vec2<i32>(0), host_size() - 1), 0);
}

// The texel under `t` (0..1 across the raster).
fn host_nearest(t: vec2<f32>) -> vec4<f32> {
    return host_load(vec2<i32>(floor(t * vec2<f32>(host_size()))));
}

fn host_bilinear(t: vec2<f32>) -> vec4<f32> {
    let f = t * vec2<f32>(host_size()) - 0.5;
    let i = vec2<i32>(floor(f));
    let w = fract(f);
    let a = mix(host_load(i), host_load(i + vec2<i32>(1, 0)), w.x);
    let b = mix(host_load(i + vec2<i32>(0, 1)), host_load(i + vec2<i32>(1, 1)), w.x);
    return mix(a, b, w.y);
}

// Bilinear, averaged over a footprint `extent` wide in raster units (0..1) so shrinking stays
// smooth: up to 4x4 taps.
fn host_smooth(t: vec2<f32>, extent: vec2<f32>) -> vec4<f32> {
    let texels = abs(extent) * vec2<f32>(host_size());
    let n = vec2<i32>(clamp(ceil(texels), vec2<f32>(1.0), vec2<f32>(4.0)));
    if n.x == 1 && n.y == 1 {
        return host_bilinear(t);
    }
    var sum = vec4<f32>(0.0);
    for (var y = 0; y < n.y; y++) {
        for (var x = 0; x < n.x; x++) {
            let o = (vec2<f32>(f32(x), f32(y)) + 0.5) / vec2<f32>(n) - 0.5;
            sum += host_bilinear(t + o * extent);
        }
    }
    return sum / f32(n.x * n.y);
}

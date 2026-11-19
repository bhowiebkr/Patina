// Node thumbnail: one layer of the node's output. Image nodes are box-filtered down to the
// thumbnail; mesh-domain nodes are shown on the model through the view map (u, v, light,
// coverage from the 3/4 preview camera), 2x2 supersampled. Values are sRGB-encoded (gray shows
// as-is) and egui shows texture bytes as gamma, so they go to a plain Rgba8Unorm target
// unconverted. thumb.x: 1 mesh domain, thumb.y: layer, thumb.z: thumbnail side.

@group(0) @binding(0) var<uniform> thumb: vec4<u32>;
@group(0) @binding(1) var src: texture_2d_array<f32>;
@group(0) @binding(2) var view_map: texture_2d<f32>;

const BACKGROUND = vec3<f32>(0.11);

@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
}

fn on_mesh(pixel: vec2<i32>, size: vec2<i32>, layer: i32) -> vec3<f32> {
    let view_size = vec2<i32>(textureDimensions(view_map));
    let cell = max(view_size / i32(thumb.z), vec2<i32>(1));
    var sum = vec3<f32>(0.0);
    for (var y = 0; y < 2; y += 1) {
        for (var x = 0; x < 2; x += 1) {
            let q = min(pixel * cell + vec2<i32>(x, y) * cell / 2, view_size - 1);
            let seen = textureLoad(view_map, q, 0);
            if seen.w > 0.5 {
                let t = clamp(vec2<i32>(seen.xy * vec2<f32>(size)), vec2<i32>(0), size - 1);
                sum += clamp(textureLoad(src, t, layer, 0).rgb, vec3<f32>(0.0), vec3<f32>(1.0))
                    * seen.z;
            } else {
                sum += BACKGROUND;
            }
        }
    }
    return sum / 4.0;
}

@fragment
fn fs(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let size = vec2<i32>(textureDimensions(src));
    let layer = i32(thumb.y);
    if thumb.x != 0u {
        return vec4<f32>(on_mesh(vec2<i32>(frag.xy), size, layer), 1.0);
    }
    let cell = max(size / i32(thumb.z), vec2<i32>(1));
    let origin = vec2<i32>(frag.xy) * cell;
    var sum = vec3<f32>(0.0);
    for (var y = 0; y < 4; y += 1) {
        for (var x = 0; x < 4; x += 1) {
            let q = min(origin + vec2<i32>(x, y) * cell / 4, size - 1);
            sum += textureLoad(src, q, layer, 0).rgb;
        }
    }
    return vec4<f32>(clamp(sum / 16.0, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}

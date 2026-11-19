//! name: Spread
//! category: Core
//! description: Blurs a mask across the surface, widening a thin edge or crease into a soft band. Radius
//! is a fraction of the model's largest side, so it means the same on any UV layout. The blur stays
//! inside each UV chart.
//! input: input Gray "Input" primary
//! output: output Gray "Mask"
//! param: radius float(0,0.25) = 0.01 "Radius" soft
//! passes: 5
//! domain: mesh

fn inside(p: vec2<i32>) -> bool {
    return textureLoad(map_pos, mesh_texel(p), 0).w > 0.75;
}

// Texels per unit on the model here, along the image's X. Stays finite at a chart's edge.
fn density(p: vec2<i32>) -> f32 {
    let step = select(vec2<i32>(-1, 0), vec2<i32>(1, 0), inside(p + vec2<i32>(1, 0)));
    return 1.0 / max(distance(mesh_position(p), mesh_position(p + step)), 1e-6);
}

// A 3x3 tent per pass with taps twice as far apart each time. Taps off the chart are left out.
fn eval(p: vec2<i32>) -> Out {
    if pass_index() == 0u {
        scratch_store(p, vec4<f32>(gray_input(p)));
        return Out(gray_input(p));
    }
    let gap = p_radius() * info.bounds.w * density(p) / f32(1u << (passes() - pass_index()));
    var sum = 0.0;
    var weight = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let q = p + vec2<i32>(round(vec2<f32>(f32(x), f32(y)) * gap));
            if inside(q) {
                let w = f32(2 - abs(x)) * f32(2 - abs(y));
                sum += scratch_load(q).x * w;
                weight += w;
            }
        }
    }
    let value = select(scratch_load(p).x, sum / weight, weight > 0.0);
    scratch_store(p, vec4<f32>(value));
    return Out(value);
}

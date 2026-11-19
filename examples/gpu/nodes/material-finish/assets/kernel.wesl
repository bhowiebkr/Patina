//! name: Finish
//! category: Core
//! description: Completes a stack of layers. Derives the normal from the final height, so relief is
//! never mixed as normals, and writes baked occlusion into the ambient occlusion channel. Depth is the
//! height of a full 0..1 step as a fraction of the model's largest side, whatever the resolution or UVs.
//! input: material Material "Material" primary
//! output: output Material "Material"
//! param: depth float(0,0.1) = 0.01 "Depth" soft
//! param: occlusion float(0,1) = 1 "Baked occlusion"
//! domain: mesh

fn inside(p: vec2<i32>) -> bool {
    return textureLoad(map_pos, mesh_texel(p), 0).w > 0.75;
}

// Rise over run along one image axis. Central where it can be, one-sided at a chart's edge. Across a
// seam the positions are far apart, so the slope comes out near zero rather than as a false ridge.
fn slope(p: vec2<i32>, step: vec2<i32>) -> f32 {
    let a = select(p, p - step, inside(p - step));
    let b = select(p, p + step, inside(p + step));
    let run = distance(mesh_position(a), mesh_position(b));
    if run < 1e-7 {
        return 0.0;
    }
    return (fetch(input0, 0u, b, 1u).z - fetch(input0, 0u, a, 1u).z) * p_depth() * info.bounds.w / run;
}

fn eval(p: vec2<i32>) -> Out {
    var m = material_material(p);
    // Rows run down the image and the normal's Y runs up it.
    m.normal = normalize(vec3<f32>(-slope(p, vec2<i32>(1, 0)), slope(p, vec2<i32>(0, 1)), 1.0)) * 0.5 + vec3<f32>(0.5);
    m.ao *= mix(1.0, mesh_ao(p), p_occlusion());
    return Out(m);
}

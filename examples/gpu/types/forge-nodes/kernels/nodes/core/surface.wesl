//! name: Surface
//! category: Mesh Masks
//! description: One fact about this point of the surface as a mask, read straight from the baked
//! maps. 1 where the fact holds most: enclosed by other surfaces, in a crease, on an edge, facing
//! along an axis, far along an axis, far from an axis, in a thick part of the model, or close to a
//! different part.
//! output: output Gray "Mask"
//! param: fact enum(Enclosed|Crease|Edge|Facing|Along|Radius|Thick|Near) = enclosed "Fact"
//! param: axis enum(X|Y|Z) = y "Axis"
//! param: gain float(0,16) = 1 "Gain" soft
//! domain: mesh

fn eval(p: vec2<i32>) -> Out {
    var value: f32;
    switch p_fact() {
        case FACT_ENCLOSED: { value = 1.0 - mesh_ao(p); }
        case FACT_CREASE: { value = (0.5 - mesh_curvature(p)) * 2.0; }
        case FACT_EDGE: { value = (mesh_curvature(p) - 0.5) * 2.0; }
        case FACT_FACING: { value = mesh_normal(p)[p_axis()] * 0.5 + 0.5; }
        case FACT_ALONG: { value = mesh_unit_position(p)[p_axis()]; }
        case FACT_RADIUS: {
            var q = mesh_unit_position(p) - vec3<f32>(0.5);
            q[p_axis()] = 0.0;
            value = length(q) * 2.0;
        }
        case FACT_NEAR: { value = 1.0 - mesh_near(p); }
        default: { value = mesh_thickness(p); }
    }
    return Out(clamp(value * p_gain(), 0.0, 1.0));
}

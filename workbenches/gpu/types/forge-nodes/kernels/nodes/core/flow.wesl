//! name: Flow
//! category: Core
//! description: Carries a mask along the surface, for leaks, drips and streaks. Each point looks back up
//! its path along an axis and keeps the strongest value it finds, fading with distance. Length is the fade
//! distance as a fraction of the model's largest side. A soft source then a Mask gives tapered streaks.
//! Paths stop at UV chart edges and on faces square to the axis.
//! input: source Gray "Source" primary
//! output: output Gray "Mask"
//! param: axis enum(X|Y|Z) = y "Axis"
//! param: direction enum(Down|Up) = down "Runs"
//! param: length float(0,1) = 0.1 "Length" soft
//! param: reach int(1,12) = 9 "Reach (2^n texels)"
//! passes: reach
//! domain: mesh

// A tangent shorter than this means the surface is nearly square to the axis, so nothing runs along it.
const MIN_SLOPE: f32 = 0.15;

fn inside(p: vec2<i32>) -> bool {
    return textureLoad(map_pos, mesh_texel(p), 0).w > 0.75;
}

// Position change per texel along one image axis, one-sided at a chart's edge.
fn tangent(p: vec2<i32>, step: vec2<i32>) -> vec3<f32> {
    let a = select(p, p - step, inside(p - step));
    let b = select(p, p + step, inside(p + step));
    let span = f32(abs(b.x - a.x) + abs(b.y - a.y));
    return (mesh_position(b) - mesh_position(a)) / max(span, 1.0);
}

// One texel's step back up the path, in texels, and its length on the model.
fn first_step(p: vec2<i32>) -> vec3<f32> {
    let dx = tangent(p, vec2<i32>(1, 0));
    let dy = tangent(p, vec2<i32>(0, 1));
    // Solve dx * u + dy * v = the axis projected on the surface.
    var a = vec3<f32>(0.0);
    a[p_axis()] = select(1.0, -1.0, p_direction() == DIRECTION_UP);
    let xx = dot(dx, dx);
    let xy = dot(dx, dy);
    let yy = dot(dy, dy);
    let det = xx * yy - xy * xy;
    if abs(det) < 1e-20 {
        return vec3<f32>(0.0);
    }
    var d = vec2<f32>(yy * dot(dx, a) - xy * dot(dy, a), xx * dot(dy, a) - xy * dot(dx, a)) / det;
    if length(dx * d.x + dy * d.y) < MIN_SLOPE {
        return vec3<f32>(0.0);
    }
    d /= max(abs(d.x), abs(d.y));
    return vec3<f32>(d, length(dx * d.x + dy * d.y));
}

// Scratch: r value, gb offset to the end of this point's path so far, a that path's length. Each pass joins
// a path to the one starting at its end, so paths double in length and n passes cover 2^n texels.
fn eval(p: vec2<i32>) -> Out {
    if pass_index() == 0u {
        let start = select(vec3<f32>(0.0), first_step(p), inside(p));
        scratch_store(p, vec4<f32>(gray_source(p), start));
        return Out(gray_source(p));
    }
    let here = scratch_load(p);
    let end = vec2<i32>(round(vec2<f32>(p) + here.yz));
    if !inside(end) {
        scratch_store(p, here);
        return Out(here.x);
    }
    let there = scratch_load(end);
    let fade = exp(-here.w / max(p_length() * info.bounds.w, 1e-5));
    let value = max(here.x, there.x * fade);
    scratch_store(p, vec4<f32>(value, here.yz + there.yz, here.w + there.w));
    return Out(value);
}

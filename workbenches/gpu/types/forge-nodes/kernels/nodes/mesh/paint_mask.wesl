//! name: Paint Mask
//! category: Mesh
//! description: A hand-painted mask, like a paint layer's mask: pick it in the Paint tab and paint it on the model with the brush, eraser or part fill. Each asset keeps its own painting.
//! output: output Gray "Mask"
//! param: invert bool = false "Invert"
//! paint
//! domain: mesh

fn eval(p: vec2<i32>) -> Out {
    let m = textureLoad(scratch_in, p, 0).r;
    return Out(select(m, 1.0 - m, p_invert()));
}

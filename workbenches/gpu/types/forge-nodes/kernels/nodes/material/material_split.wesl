//! name: Material Split
//! category: Material
//! description: Splits a material into its PBR channels.
//! input: material Material "Material" primary
//! output: basecolor Color "Base color"
//! output: roughness Gray "Roughness"
//! output: metallic Gray "Metallic"
//! output: height Gray "Height"
//! output: ao Gray "Ambient occlusion"
//! output: normal Color "Normal"

fn eval(p: vec2<i32>) -> Out {
    let m = material_material(p);
    return Out(m.basecolor, m.roughness, m.metallic, m.height, m.ao, vec4<f32>(m.normal, 1.0));
}

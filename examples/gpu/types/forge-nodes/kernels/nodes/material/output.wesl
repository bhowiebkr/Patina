//! name: Output
//! category: Material
//! description: The finished material shown in the viewport and exported.
//! input: material Material "Material" primary
//! output: result Material "Material"
//! domain: mesh

fn eval(p: vec2<i32>) -> Out {
    return Out(material_material(p));
}

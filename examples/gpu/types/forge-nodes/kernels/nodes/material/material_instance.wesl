//! name: Material Instance
//! category: Material
//! description: Another saved material as one node: its exposed parameters become this node's
//! settings, and editing that material updates every instance of it.
//! input: source Material "Material" primary hidden
//! output: material Material "Material"
//! param: reference text = "" "Material"

fn eval(p: vec2<i32>) -> Out {
    return Out(material_source(p));
}

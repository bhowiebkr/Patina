//! name: Material graph port
//! category: Subgraphs
//! description: A declared subgraph boundary or its unconnected fallback material.
//! input: source Material "Source"
//! output: output Material
//! domain: inherit

fn eval(p: vec2<i32>) -> Out { return Out(material_source(p)); }

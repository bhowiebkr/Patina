//! name: Color graph port
//! category: Subgraphs
//! description: A declared subgraph boundary or its unconnected fallback value.
//! input: source Color "Source" default=(0,0,0,1)
//! output: output Color
//! domain: inherit

fn eval(p: vec2<i32>) -> Out { return Out(in_source(p)); }

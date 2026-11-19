//! name: Grayscale graph port
//! category: Subgraphs
//! description: A declared subgraph boundary or its unconnected fallback value.
//! input: source Gray "Source" default=0
//! output: output Gray
//! domain: inherit

fn eval(p: vec2<i32>) -> Out { return Out(gray_source(p)); }

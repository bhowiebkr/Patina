//! Smart-material node graphs: the graph model (saved as JSON), node definitions read from
//! `kernels/nodes/<category>/<id>.wesl` (a `//!` header declares inputs, outputs and
//! parameters; the body is the WESL `eval`), and the GPU engine that evaluates a graph over an
//! asset's baked mesh maps. Definitions and the shared modules hot-reload while the app runs.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
mod canvas;
mod controls;
pub use controls::PresetError;
mod codegen;
mod def;
mod engine;
mod gpu;
mod graph;
mod host;
mod link;
mod paint;
mod preset;
mod registry;
pub mod subgraph;

pub use canvas::{Canvas, Frame, Note, ParameterPreset};
pub use codegen::generate;
pub use def::{
    CURVE_CONTROL_POINTS, CURVE_POINTS, Def, DefError, Domain, GRADIENT_STOPS, Host, Input, Output,
    Param, ParamType, Socket, TextKind, curve_values, sort_stops, title_case,
};
pub use engine::{Engine, GraphState, NativeImage, NativePass, TEXTURE_SIZE};
pub use gpu::{
    OUT_TEXEL, THUMB, THUMB_FORMAT, device_descriptor, out_texel, required_features,
    required_limits,
};
pub use graph::{Exposed, Graph, Link, Node, NodeId, Size, VERSION};
pub use paint::{Brush, MASK_TEXEL, Tool};
pub use preset::{blank_material, sample_material};
pub use registry::Registry;

//! Small wgpu helpers shared by the renderer and the node engine.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
mod shader;
mod texture;

pub use shader::{PACKAGED, ShaderFile, compile, finish_scope, validated};
pub use texture::{Target, read_texture, read_texture_region, uniform_buffer, write_texture};
#[cfg(target_arch = "wasm32")]
pub use texture::{read_texture_async, read_texture_region_async};

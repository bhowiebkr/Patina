//! Viewport rendering: an orbit camera, a forward PBR renderer drawing one asset into an
//! offscreen view the UI shows as an image, and the mesh maps (position, normal, curvature,
//! AO, thickness) baked in UV space for the node engine.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
mod bvh;
mod camera;
mod compute;
mod curvature;
mod environment;
#[cfg(test)]
mod gpu_test;
mod maps;
mod occlusion;
mod pick;
mod planes;
mod scene;
mod surface;
mod view_map;

pub use camera::OrbitCamera;
pub use environment::Environment;
pub use maps::{BakeOptions, MeshMaps};
pub use pick::Picker;
pub use scene::{COLOR, GpuAsset, Light, SceneRenderer, View};

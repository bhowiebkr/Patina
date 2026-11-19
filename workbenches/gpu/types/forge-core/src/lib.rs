//! Project data shared by every Millwright crate. No GPU, no UI.
//!
//! A project is a folder: `millwright.json` (manifest), `assets/<id>/asset.json` + `mesh.bin`
//! (+ `anim.json`), `materials/<id>.graph.json`. Paths stored in files are project-relative.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
pub mod anim;
pub mod asset;
pub mod error;
pub mod mesh;
pub mod project;
pub mod uv;

pub use anim::{Clip, Track};
pub use asset::{AssetInfo, Bounds, FittedView, Group, PartInfo, PartMaterial};
pub use error::{Error, Result};
pub use mesh::{MeshData, Vertex};
pub use project::{Manifest, Project};
pub use uv::{UvSettings, fnv1a};

/// A path as people write it: Windows' `\\?\` prefix from canonicalizing is dropped.
pub fn display_path(path: &std::path::Path) -> String {
    let text = path.display().to_string();
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = text.strip_prefix(r"\\?\") {
        rest.to_owned()
    } else {
        text
    }
}

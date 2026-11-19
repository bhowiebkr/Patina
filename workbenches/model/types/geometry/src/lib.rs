//! The geometry types of the model workbench: solids from the Manifold kernel, meshes, sockets
//! and seeds. The kernel is C++ in `vendor/`, and `build.rs` compiles it.

mod mesh;
mod seed;
mod sockets;
mod solid;
mod view;

use std::fmt::{self, Write};

use patina_sdk::{Text, Thumbnail, Ui, Viewer};

pub use mesh::Mesh;
pub use seed::{Random, Seed};
pub use sockets::{Frame, Sockets};
pub use solid::Solid;

/// The side of the thumbnail and of the larger view of the **Inspector**, in pixels.
const THUMBNAIL: usize = 64;
const LARGE: usize = 192;

/// Draws the mesh in a square of `size` pixels.
fn picture(mesh: &Mesh, size: usize) -> Vec<u8> {
    let mut rgba = vec![0; size * size * 4];
    view::draw(mesh, size, &mut rgba);
    rgba
}

/// The **Inspector** shows a solid wire as the triangle count and the bounds, and as a picture.
impl Viewer for Solid {
    fn text(&self, out: &mut Text<'_>) -> fmt::Result {
        write!(out, "{self:?}")
    }

    fn thumbnail(&self, out: &mut Thumbnail<'_>) {
        view::draw(&self.mesh(), THUMBNAIL, out.pixels(THUMBNAIL as u32, THUMBNAIL as u32));
    }

    fn show(&self, ui: &mut Ui) {
        ui.image("Solid", [LARGE as u32; 2], &picture(&self.mesh(), LARGE));
    }
}

/// Shows "12 triangles, bounds -1 -1 -1 to 1 1 1".
impl Viewer for Mesh {
    fn text(&self, out: &mut Text<'_>) -> fmt::Result {
        let mut bounds = [[f64::INFINITY; 3], [f64::NEG_INFINITY; 3]];
        for p in &self.positions {
            for i in 0..3 {
                bounds[0][i] = bounds[0][i].min(f64::from(p[i]));
                bounds[1][i] = bounds[1][i].max(f64::from(p[i]));
            }
        }
        write!(out, "{}", Summary(self.triangles(), bounds))
    }

    fn thumbnail(&self, out: &mut Thumbnail<'_>) {
        view::draw(self, THUMBNAIL, out.pixels(THUMBNAIL as u32, THUMBNAIL as u32));
    }

    fn show(&self, ui: &mut Ui) {
        ui.image("Mesh", [LARGE as u32; 2], &picture(self, LARGE));
    }
}

/// Shows the names of the sockets and the origin of each socket.
impl Viewer for Sockets {
    fn text(&self, out: &mut Text<'_>) -> fmt::Result {
        write!(out, "{self:?}")
    }

    fn show(&self, ui: &mut Ui) {
        for (name, frame) in self.iter() {
            let [x, y, z] = frame.origin;
            ui.label(&format!("{name} at {x} {y} {z}"));
        }
    }
}

impl Viewer for Seed {
    fn text(&self, out: &mut Text<'_>) -> fmt::Result {
        write!(out, "{self:?}")
    }
}

/// The text of `mesh::summary` for a `Display`.
struct Summary(usize, [[f64; 3]; 2]);

impl fmt::Display for Summary {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        mesh::summary(f, self.0, self.1)
    }
}

#[cfg(test)]
mod tests;

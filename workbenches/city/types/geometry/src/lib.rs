//! The geometry types of the model workbench: solids and sketches from the Manifold kernel,
//! meshes, sockets and seeds. The kernel is C++ in `vendor/`, and `build.rs` compiles it.

pub mod export;
mod loft;
mod mesh;
mod scene;
mod seed;
mod sketch;
mod sockets;
mod solid;
mod view;

use std::fmt::{self, Write};

use patina_sdk::{Text, Thumbnail, Ui, Viewer};

pub use mesh::Mesh;
pub use scene::{IDENTITY, Item, Matrix, Place, Scene};
pub use seed::{Random, Seed};
pub use sketch::Sketch;
pub use sockets::{Frame, Sockets};
pub use solid::{MAX_COPIES, MAX_SEGMENTS, Solid};

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

/// The **Inspector** shows a sketch as a thin plate, with the corner count and the area.
impl Viewer for Sketch {
    fn text(&self, out: &mut Text<'_>) -> fmt::Result {
        write!(out, "{self:?}")
    }

    fn thumbnail(&self, out: &mut Thumbnail<'_>) {
        view::draw(&plate(self).mesh(), THUMBNAIL, out.pixels(THUMBNAIL as u32, THUMBNAIL as u32));
    }

    fn show(&self, ui: &mut Ui) {
        ui.image("Sketch", [LARGE as u32; 2], &picture(&plate(self).mesh(), LARGE));
    }
}

/// The sketch with a thickness of 5 % of its size, to draw it.
fn plate(sketch: &Sketch) -> Solid {
    let [[x0, y0], [x1, y1]] = sketch.bounds();
    let size = (x1 - x0).max(y1 - y0);
    Solid::extrude(sketch, if size.is_finite() && size > 0.0 { size * 0.05 } else { 1.0 }, 0.0, [1.0; 2], 0)
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

/// Shows the counts and the bounds. The 3D view draws a scene.
impl Viewer for Scene {
    fn text(&self, out: &mut Text<'_>) -> fmt::Result {
        write!(out, "{self:?}")
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
mod measure;
#[cfg(test)]
mod tests;

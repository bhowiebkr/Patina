//! The 2D transform widget of a node GUI (spec 4.3): a gizmo on an image, with number fields.

use super::{Response, Ui, push_pixels};
use std::fmt::Write;

/// The scale limits of a transform, so that a drag cannot make a scale of 0.
const MIN_SCALE: f32 = 0.01;
const MAX_SCALE: f32 = 100.0;

/// A move, a rotation and a scale around a pivot, for example of a layer on an image, in the
/// pixels of a frame. `Ui::transform` edits it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform2d {
    /// The move in pixels.
    pub offset: [f32; 2],
    /// The rotation in degrees, from -180 to 180. A positive angle turns clockwise on the screen.
    pub rotation: f32,
    pub scale: [f32; 2],
    /// The point that the rotation and the scale keep, from 0 to 1 in the frame.
    pub pivot: [f32; 2],
    /// The aspect lock: a drag of a corner keeps the ratio of the two scales.
    pub lock: bool,
}

impl Default for Transform2d {
    fn default() -> Transform2d {
        Transform2d { offset: [0.0; 2], rotation: 0.0, scale: [1.0; 2], pivot: [0.5; 2], lock: true }
    }
}

impl Transform2d {
    /// The place of the point `p` of a frame of `size` pixels after the transform. The y axis
    /// goes down, as in an image.
    pub fn apply(&self, size: [f32; 2], p: [f32; 2]) -> [f32; 2] {
        let (sin, cos) = self.rotation.to_radians().sin_cos();
        let pivot = [self.pivot[0] * size[0], self.pivot[1] * size[1]];
        let [x, y] = [(p[0] - pivot[0]) * self.scale[0], (p[1] - pivot[1]) * self.scale[1]];
        [pivot[0] + x * cos - y * sin + self.offset[0], pivot[1] + x * sin + y * cos + self.offset[1]]
    }

    /// The point of the frame that the transform puts at `p`, for example to find the source
    /// pixel of each pixel of a transformed image.
    pub fn invert(&self, size: [f32; 2], p: [f32; 2]) -> [f32; 2] {
        let (sin, cos) = self.rotation.to_radians().sin_cos();
        let pivot = [self.pivot[0] * size[0], self.pivot[1] * size[1]];
        let [x, y] = [p[0] - pivot[0] - self.offset[0], p[1] - pivot[1] - self.offset[1]];
        [pivot[0] + (x * cos + y * sin) / self.scale[0], pivot[1] + (y * cos - x * sin) / self.scale[1]]
    }
}

/// Reads a transform event: `set <x> <y> <rotation> <scale x> <scale y> <pivot x> <pivot y> <lock>`.
fn edit(event: &str) -> Option<Transform2d> {
    let mut words = event.strip_prefix("set ")?.split(' ');
    let mut next = || words.next()?.parse::<f32>().ok().filter(|v| v.is_finite());
    let offset = [next()?, next()?];
    let rotation = next()? % 360.0;
    let scale = [next()?, next()?].map(|s| s.clamp(MIN_SCALE, MAX_SCALE));
    let pivot = [next()?, next()?].map(|p| p.clamp(0.0, 1.0));
    let lock = next()? != 0.0;
    let rotation = if rotation > 180.0 {
        rotation - 360.0
    } else if rotation <= -180.0 {
        rotation + 360.0
    } else {
        rotation
    };
    Some(Transform2d { offset, rotation, scale, pivot, lock })
}

impl Ui {
    /// Shows a 2D transform gizmo for `transform` on a frame of `frame` pixels, with the number
    /// fields under it. `image` is the picture of the frame behind the gizmo: its size and its
    /// RGBA pixels, as `Ui::image` takes them. The user drags the box to move it, the ring to
    /// rotate it and the corners to scale it.
    pub fn transform(&mut self, text: &str, frame: [f32; 2], transform: &mut Transform2d, image: Option<([u32; 2], &[u8])>) -> Response {
        let edit = self.push("transform", text).and_then(edit);
        let [w, h] = frame.map(|v| if v.is_finite() { v.max(1.0) } else { 1.0 });
        let t = &*transform;
        let _ = write!(
            self.tree,
            "\t{w} {h}\t{} {}\t{}\t{} {}\t{} {}\t{}",
            t.offset[0],
            t.offset[1],
            t.rotation,
            t.scale[0],
            t.scale[1],
            t.pivot[0],
            t.pivot[1],
            u8::from(t.lock)
        );
        let (size, rgba) = image.unwrap_or(([0; 2], &[]));
        push_pixels(&mut self.tree, size, rgba);
        let changed = edit.is_some_and(|e| e != *transform);
        if let Some(e) = edit {
            *transform = e;
        }
        Response { clicked: false, changed }
    }
}

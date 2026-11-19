//! `Sockets`: named frames on a model, for example `muzzle` and `grip`.

use std::fmt;

/// A position and a direction: the X, Y and Z axes and the origin of a frame, in the
/// coordinates of the model.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub x: [f64; 3],
    pub y: [f64; 3],
    pub z: [f64; 3],
    pub origin: [f64; 3],
}

impl Default for Frame {
    fn default() -> Frame {
        Frame { x: [1.0, 0.0, 0.0], y: [0.0, 1.0, 0.0], z: [0.0, 0.0, 1.0], origin: [0.0; 3] }
    }
}

impl Frame {
    /// A frame at `origin` with the axes of the model.
    pub fn at(origin: [f64; 3]) -> Frame {
        Frame { origin, ..Frame::default() }
    }
}

/// The named frames of a model. A name is in the list one time at most.
#[derive(Clone, Default, PartialEq)]
pub struct Sockets(Vec<(String, Frame)>);

impl Sockets {
    /// Sets the frame of `name`. It replaces a frame with the same name.
    pub fn set(&mut self, name: &str, frame: Frame) {
        match self.0.iter_mut().find(|(n, _)| n == name) {
            Some((_, old)) => *old = frame,
            None => self.0.push((name.to_owned(), frame)),
        }
    }

    /// The frame of `name`.
    pub fn get(&self, name: &str) -> Option<&Frame> {
        self.0.iter().find(|(n, _)| n == name).map(|(_, frame)| frame)
    }

    /// The names and the frames, in the order of their first `set`.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Frame)> {
        self.0.iter().map(|(name, frame)| (name.as_str(), frame))
    }
}

/// Shows the names, for example "2 sockets: muzzle, grip".
impl fmt::Debug for Sockets {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} socket{}", self.0.len(), if self.0.len() == 1 { "" } else { "s" })?;
        for (i, (name, _)) in self.0.iter().enumerate() {
            write!(f, "{}{name}", if i == 0 { ": " } else { ", " })?;
        }
        Ok(())
    }
}

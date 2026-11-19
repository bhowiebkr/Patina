//! `Sockets`: named frames on a model, for example `muzzle` and `grip`.

use std::fmt;

use patina_sdk::Param;

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

    /// A frame at `origin`, turned by the angles in degrees: first about X, then Y, then Z, as
    /// `Solid::rotate`.
    pub fn turned(origin: [f64; 3], [a, b, c]: [f64; 3]) -> Frame {
        let ((sa, ca), (sb, cb), (sc, cc)) = (sin_cos(a), sin_cos(b), sin_cos(c));
        // The columns of Rz(c) Ry(b) Rx(a).
        Frame {
            x: [cc * cb, sc * cb, -sb],
            y: [cc * sb * sa - sc * ca, sc * sb * sa + cc * ca, cb * sa],
            z: [cc * sb * ca + sc * sa, sc * sb * ca - cc * sa, cb * ca],
            origin,
        }
    }

    /// The angles of `turned`, in degrees.
    pub fn angles(&self) -> [f64; 3] {
        let b = (-self.x[2]).clamp(-1.0, 1.0).asin().to_degrees();
        if self.x[2].abs() > 1.0 - 1e-12 {
            // At b = 90 or -90 degrees, only the sum of the X and Z turns is known: put it on Z.
            return [0.0, b, (-self.y[0]).atan2(self.y[1]).to_degrees()];
        }
        [self.y[2].atan2(self.z[2]).to_degrees(), b, self.x[1].atan2(self.x[0]).to_degrees()]
    }
}

/// The sine and the cosine of an angle in degrees, exact at multiples of 90 degrees as in the kernel.
fn sin_cos(degrees: f64) -> (f64, f64) {
    let turn = degrees.rem_euclid(360.0);
    match turn {
        0.0 => (0.0, 1.0),
        90.0 => (1.0, 0.0),
        180.0 => (0.0, -1.0),
        270.0 => (-1.0, 0.0),
        _ => degrees.to_radians().sin_cos(),
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

/// A sockets input with no wire is a text in the **Inspector**: one "name x y z" item on each
/// line or after a ";". Three more numbers turn the frame, for example "top 0 0 2 0 0 90".
impl Param for Sockets {
    fn default_value() -> Sockets {
        Sockets::default()
    }

    fn from_text(text: &str) -> Result<Sockets, String> {
        let mut sockets = Sockets::default();
        for item in text.split([';', '\n']).map(str::trim).filter(|item| !item.is_empty()) {
            let mut words = item.split_whitespace();
            let name = words.next().unwrap_or_default();
            let number = |word: &str| word.parse::<f64>().ok().filter(|n| n.is_finite());
            let numbers: Vec<f64> =
                words.map(|w| number(w).ok_or_else(|| format!("{w:?} in {item:?} is not a number"))).collect::<Result<_, _>>()?;
            let frame = match numbers[..] {
                [x, y, z] => Frame::at([x, y, z]),
                [x, y, z, a, b, c] => Frame::turned([x, y, z], [a, b, c]),
                _ => return Err(format!("write {item:?} as a name and 3 or 6 numbers")),
            };
            sockets.set(name, frame);
        }
        Ok(sockets)
    }

    fn to_text(&self) -> String {
        let items: Vec<String> = self
            .iter()
            .map(|(name, frame)| {
                let [x, y, z] = frame.origin;
                if Frame::at(frame.origin) == *frame {
                    return format!("{name} {x} {y} {z}");
                }
                let [a, b, c] = frame.angles();
                format!("{name} {x} {y} {z} {a} {b} {c}")
            })
            .collect();
        items.join("; ")
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

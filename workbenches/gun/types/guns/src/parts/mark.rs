//! The maker mark on the two sides of a receiver: the logo of the maker and its name in raised
//! letters of the trim color. Each side reads from left to right. The serial number of the gun on
//! the right side.

use std::f64::consts::PI;

use super::Build;
use crate::shapes::decal;
use crate::{Choice, Maker, Material};

/// The width of a stroke of a letter, in units of the letter grid. A letter is 4 x 6 units.
const STROKE: f64 = 0.9;
/// The distance from one letter to the next, in grid units.
const ADVANCE: f64 = 6.2;
/// The width of the logo and the space after it, in grid units. The logo is 6 x 6 units.
const LOGO: f64 = 8.0;

/// Adds the mark of `maker` on the faces `y = side` and `y = -side`. The mark starts at `x[0]`
/// with its base at `z`. Its letters are `height` high, or less to stay in `x`.
pub(super) fn mark(b: &mut Build, maker: Maker, x: [f64; 2], z: f64, height: f64, side: f64) {
    let name = maker.name();
    let wide = LOGO + width(name);
    let unit = (height / (6.0 + STROKE)).min((x[1] - x[0]) / wide);
    let mut shapes = logo(maker);
    shapes.extend(text(name, LOGO));
    // The +Y side is seen from the other direction, so its mark is mirrored along X.
    for (side, flip) in [(-side, false), (side, true)] {
        for shape in &shapes {
            let profile: Vec<[f64; 2]> = shape.iter().map(|&[u, v]| [x[0] + unit * if flip { wide - u } else { u }, z + unit * v]).collect();
            b.add(Material::Trim, decal(&profile, side, 0.0008));
        }
    }
}

/// Adds the serial number of the gun in `material` on the right face `y = -side`. The number
/// starts at `x[0]` with its base at `z`. Its digits are `height` high, or less to stay in `x`.
pub(super) fn serial(b: &mut Build, material: Material, x: [f64; 2], z: f64, height: f64, side: f64) {
    let number = format!("{:06}", b.seed.child("serial").random().index(1_000_000));
    let unit = (height / (6.0 + STROKE)).min((x[1] - x[0]) / width(&number));
    for shape in text(&number, 0.0) {
        let profile: Vec<[f64; 2]> = shape.iter().map(|&[u, v]| [x[0] + unit * u, z + unit * v]).collect();
        b.add(material, decal(&profile, -side, 0.0006));
    }
}

/// The width of `text` in grid units, with the strokes.
fn width(text: &str) -> f64 {
    ADVANCE * (text.len() - 1) as f64 + 4.0 + STROKE
}

/// The strokes of `text` in grid units, with the left edge of the first letter at `left`.
fn text(text: &str, left: f64) -> Vec<Vec<[f64; 2]>> {
    let mut shapes = Vec::new();
    for (i, c) in text.chars().enumerate() {
        let left = left + STROKE / 2.0 + ADVANCE * i as f64;
        for line in letter(c) {
            for pair in line.windows(2) {
                let at = |[u, v]: [i8; 2]| [left + f64::from(u), STROKE / 2.0 + f64::from(v)];
                shapes.push(stroke(at(pair[0]), at(pair[1])));
            }
        }
    }
    shapes
}

/// The strokes of each letter of the maker names and of each digit, as lines through points of
/// the grid.
fn letter(c: char) -> &'static [&'static [[i8; 2]]] {
    const P: &[[i8; 2]] = &[[0, 0], [0, 6], [3, 6], [4, 5], [4, 4], [3, 3], [0, 3]];
    const O: &[[i8; 2]] = &[[1, 0], [0, 1], [0, 5], [1, 6], [3, 6], [4, 5], [4, 1], [3, 0], [1, 0]];
    match c {
        '0' => &[O, &[[1, 1], [3, 5]]],
        '1' => &[&[[1, 5], [2, 6], [2, 0]], &[[1, 0], [3, 0]]],
        '2' => &[&[[0, 5], [1, 6], [3, 6], [4, 5], [4, 4], [0, 0], [4, 0]]],
        '3' => &[&[[0, 5], [1, 6], [3, 6], [4, 5], [4, 4], [3, 3], [4, 2], [4, 1], [3, 0], [1, 0], [0, 1]], &[[1, 3], [3, 3]]],
        '4' => &[&[[3, 0], [3, 6], [0, 2], [4, 2]]],
        '5' => &[&[[4, 6], [0, 6], [0, 3], [3, 3], [4, 2], [4, 1], [3, 0], [0, 0]]],
        '6' => &[&[[4, 5], [3, 6], [1, 6], [0, 5], [0, 1], [1, 0], [3, 0], [4, 1], [4, 2], [3, 3], [0, 3]]],
        '7' => &[&[[0, 6], [4, 6], [1, 0]]],
        '8' => &[&[[1, 3], [0, 4], [0, 5], [1, 6], [3, 6], [4, 5], [4, 4], [3, 3], [1, 3], [0, 2], [0, 1], [1, 0], [3, 0], [4, 1], [4, 2], [3, 3]]],
        '9' => &[&[[0, 1], [1, 0], [3, 0], [4, 1], [4, 5], [3, 6], [1, 6], [0, 5], [0, 4], [1, 3], [4, 3]]],
        'a' => &[&[[0, 0], [0, 4], [2, 6], [4, 4], [4, 0]], &[[0, 3], [4, 3]]],
        'b' => &[P, &[[3, 3], [4, 2], [4, 1], [3, 0], [0, 0]]],
        'c' => &[&[[4, 5], [3, 6], [1, 6], [0, 5], [0, 1], [1, 0], [3, 0], [4, 1]]],
        'e' => &[&[[4, 6], [0, 6], [0, 0], [4, 0]], &[[0, 3], [3, 3]]],
        'h' => &[&[[0, 0], [0, 6]], &[[4, 0], [4, 6]], &[[0, 3], [4, 3]]],
        'j' => &[&[[4, 6], [4, 1], [3, 0], [1, 0], [0, 1]]],
        'k' => &[&[[0, 0], [0, 6]], &[[4, 6], [0, 2]], &[[1, 3], [4, 0]]],
        'l' => &[&[[0, 6], [0, 0], [4, 0]]],
        'o' => &[O],
        'p' => &[P],
        'r' => &[P, &[[2, 3], [4, 0]]],
        's' => &[&[[4, 5], [3, 6], [1, 6], [0, 5], [0, 4], [1, 3], [3, 3], [4, 2], [4, 1], [3, 0], [1, 0], [0, 1]]],
        'v' => &[&[[0, 6], [2, 0], [4, 6]]],
        'w' => &[&[[0, 6], [1, 0], [2, 4], [3, 0], [4, 6]]],
        _ => &[],
    }
}

/// A stroke from `a` to `b` with square ends, so that the strokes of a line join at the corners.
fn stroke(a: [f64; 2], b: [f64; 2]) -> Vec<[f64; 2]> {
    let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
    let k = STROKE / 2.0 / dx.hypot(dz);
    let (ux, uz) = (dx * k, dz * k);
    vec![[a[0] - ux + uz, a[1] - uz - ux], [b[0] + ux + uz, b[1] + uz - ux], [b[0] + ux - uz, b[1] + uz + ux], [a[0] - ux - uz, a[1] - uz + ux]]
}

/// The logo of the maker in the square from 0 to 6, as profiles.
fn logo(maker: Maker) -> Vec<Vec<[f64; 2]>> {
    let square = |x: f64, z: f64| vec![[x, z], [x + 2.8, z], [x + 2.8, z + 2.8], [x, z + 2.8]];
    match maker {
        // Three stacked blocks.
        Maker::Blockwerk => vec![square(0.0, 0.0), square(3.2, 0.0), square(1.6, 3.2)],
        // A crescent: the outer arc of a circle and the inner arc of a circle more to the right.
        Maker::Vesper => {
            let (a, c) = (50f64.to_radians(), 4.2);
            let (ex, ez) = (3.0 + 3.0 * a.cos(), 3.0 + 3.0 * a.sin());
            let (r, b) = ((ex - c).hypot(ez - 3.0), (ez - 3.0).atan2(ex - c));
            let mut moon = arc([3.0, 3.0], 3.0, a, 2.0 * PI - a);
            // The ends of the inner arc are the ends of the outer arc, so they are left out.
            moon.extend(&arc([c, 3.0], r, 2.0 * PI - b, b)[1..16]);
            vec![moon]
        }
        // An upper and a lower jaw with teeth that do not touch.
        Maker::Scrapjaw => {
            let teeth = |z: [f64; 2]| (0..9).map(move |i| [6.0 - 0.75 * f64::from(i), if i % 2 == 0 { z[0] } else { z[1] }]);
            let mut upper = vec![[0.0, 6.0], [6.0, 6.0]];
            upper.extend(teeth([3.8, 2.8]));
            let mut lower = vec![[0.0, 0.0], [6.0, 0.0]];
            lower.extend(teeth([3.2, 2.2]).map(|[x, z]| if (x - 6.0).abs() < 1e-9 || x.abs() < 1e-9 { [x, 2.2] } else { [x, z] }));
            vec![upper, lower]
        }
        // A ring in two arcs, with a gap on the left and on the right.
        Maker::Halo => [15f64, 195.0]
            .into_iter()
            .map(|start| {
                let (a0, a1) = (start.to_radians(), (start + 150.0).to_radians());
                let mut half = arc([3.0, 3.0], 3.0, a0, a1);
                half.extend(arc([3.0, 3.0], 2.0, a1, a0));
                half
            })
            .collect(),
    }
}

/// The points on a circle from the angle `a0` to `a1`, in radians.
fn arc(c: [f64; 2], r: f64, a0: f64, a1: f64) -> Vec<[f64; 2]> {
    (0..=16).map(|i| a0 + (a1 - a0) * f64::from(i) / 16.0).map(|t| [c[0] + r * t.cos(), c[1] + r * t.sin()]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_maker_name_and_digit_has_letters_and_a_logo() {
        for &maker in Maker::ALL {
            for c in maker.name().chars().chain('0'..='9') {
                let lines = letter(c);
                assert!(!lines.is_empty(), "{c}");
                // Each point is on the grid of 4 x 6.
                assert!(lines.iter().flat_map(|line| line.iter()).all(|&[u, v]| (0..=4).contains(&u) && (0..=6).contains(&v)), "{c}");
            }
            let shapes = logo(maker);
            assert!(!shapes.is_empty(), "{maker:?}");
            assert!(shapes.iter().flatten().all(|&[x, z]| (-1e-9..=6.0 + 1e-9).contains(&x) && (-1e-9..=6.0 + 1e-9).contains(&z)), "{maker:?}");
        }
    }

    #[test]
    fn a_stroke_is_a_rectangle_around_its_line() {
        // A stroke from (0, 0) to (2, 0) goes from -STROKE / 2 to 2 + STROKE / 2 on X.
        let h = STROKE / 2.0;
        let quad = stroke([0.0, 0.0], [2.0, 0.0]);
        assert_eq!(quad.len(), 4);
        for (p, q) in quad.iter().zip([[-h, -h], [2.0 + h, -h], [2.0 + h, h], [-h, h]]) {
            assert!((p[0] - q[0]).abs() < 1e-12 && (p[1] - q[1]).abs() < 1e-12, "{p:?} {q:?}");
        }
    }
}

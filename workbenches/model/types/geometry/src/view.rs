//! A small software raster of a mesh, for the thumbnail and the **Inspector**.

use std::f32::consts::FRAC_1_SQRT_2;

use crate::mesh::Mesh;

/// The direction to the eye: from the front right and above. Front is -Y, and Z is up.
const EYE: [f32; 3] = [0.577_350_3, -0.577_350_3, 0.577_350_3];
/// The screen axes of the isometric view. `RIGHT` and `UP` are at 90 degrees to `EYE`.
const RIGHT: [f32; 3] = [FRAC_1_SQRT_2, FRAC_1_SQRT_2, 0.0];
const UP: [f32; 3] = [-0.408_248_3, 0.408_248_3, 0.816_496_6];
/// The direction to the light, normalized at use.
const LIGHT: [f32; 3] = [0.3, -0.6, 0.9];
/// The colors of the parts: the info color and the category accents of the GUI guide theme.
const COLORS: [[f32; 3]; 6] = [
    [197.0, 197.0, 191.0],
    [227.0, 184.0, 105.0],
    [121.0, 212.0, 200.0],
    [222.0, 143.0, 186.0],
    [217.0, 161.0, 122.0],
    [185.0, 155.0, 223.0],
];

/// Draws the mesh into `rgba` (`size` x `size` pixels, row by row from the top) in an isometric
/// view that fits the mesh. The background stays transparent.
pub fn draw(mesh: &Mesh, size: usize, rgba: &mut [u8]) {
    rgba.fill(0);
    let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let screen: Vec<[f32; 3]> = mesh.positions.iter().map(|&p| [dot(p, RIGHT), dot(p, UP), dot(p, EYE)]).collect();
    let (mut low, mut high) = ([f32::MAX; 2], [f32::MIN; 2]);
    for p in &screen {
        for i in 0..2 {
            low[i] = low[i].min(p[i]);
            high[i] = high[i].max(p[i]);
        }
    }
    let span = (high[0] - low[0]).max(high[1] - low[1]);
    if screen.is_empty() || span <= 0.0 || !span.is_finite() {
        return;
    }
    let side = size as f32;
    let scale = side * 0.9 / span;
    let center = [(low[0] + high[0]) / 2.0, (low[1] + high[1]) / 2.0];
    let pixel = |p: [f32; 3]| [side / 2.0 + (p[0] - center[0]) * scale, side / 2.0 - (p[1] - center[1]) * scale, p[2]];
    let length = dot(LIGHT, LIGHT).sqrt();
    let light = LIGHT.map(|x| x / length);
    let mut depth = vec![f32::MIN; size * size];
    for (tri, (corners, &part)) in screen.as_chunks::<3>().0.iter().zip(&mesh.parts).enumerate() {
        let normal = mesh.normals[tri * 3];
        if dot(normal, EYE) <= 0.0 {
            continue;
        }
        let shade = 0.35 + 0.65 * dot(normal, light).max(0.0);
        let color = COLORS[part as usize % COLORS.len()].map(|c| (c * shade) as u8);
        fill(size, [pixel(corners[0]), pixel(corners[1]), pixel(corners[2])], &mut depth, |at| {
            rgba[at * 4..at * 4 + 4].copy_from_slice(&[color[0], color[1], color[2], 255]);
        });
    }
}

/// Calls `plot` for each pixel center in the triangle that is nearer than `depth`. The corners
/// are x and y in pixels, and a larger z is nearer.
fn fill(size: usize, [a, b, c]: [[f32; 3]; 3], depth: &mut [f32], mut plot: impl FnMut(usize)) {
    let area = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
    if area.abs() < 1e-6 {
        return;
    }
    let range = |i: usize| {
        let low = a[i].min(b[i]).min(c[i]).floor().max(0.0) as usize;
        let high = (a[i].max(b[i]).max(c[i]).ceil().max(0.0) as usize).min(size);
        low..high
    };
    for y in range(1) {
        for x in range(0) {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let edge = |p: [f32; 3], q: [f32; 3]| ((q[0] - p[0]) * (py - p[1]) - (q[1] - p[1]) * (px - p[0])) / area;
            let (wa, wb, wc) = (edge(b, c), edge(c, a), edge(a, b));
            if wa < 0.0 || wb < 0.0 || wc < 0.0 {
                continue;
            }
            let z = wa * a[2] + wb * b[2] + wc * c[2];
            let at = y * size + x;
            if z > depth[at] {
                depth[at] = z;
                plot(at);
            }
        }
    }
}

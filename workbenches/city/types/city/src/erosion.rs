//! Hydraulic erosion of a terrain: drops of rain run down the slopes. A drop takes up soil where
//! it speeds up and puts it down where it slows down, so the valleys get deeper and flatter.

use geometry::Seed;

use crate::Terrain;

/// The most drops for each sample, so that a large value cannot stop the program for long.
pub const MAX_DROPS: f64 = 8.0;

/// The steps of one drop, and the radius of the area that a drop wears down, in samples.
const STEPS: usize = 40;
const RADIUS: i64 = 3;
/// The part of the old direction that a drop keeps at each step.
const INERTIA: f64 = 0.05;
/// The soil that a drop can carry for each meter of fall, each unit of speed and of water.
const CAPACITY: f64 = 4.0;
/// The smallest fall for the capacity, in meters, so a drop on flat ground carries a little soil.
const MIN_FALL: f64 = 0.01;
/// The parts of the free capacity that a drop takes up, and of the extra soil that it puts down.
const ERODE: f64 = 0.3;
const DEPOSIT: f64 = 0.3;
/// The part of the water that goes at each step, and the gain of speed for each unit of slope.
const EVAPORATE: f64 = 0.02;
const GRAVITY: f64 = 4.0;

/// The terrain after `drops` drops of rain for each sample. Each drop starts at a random point and
/// runs down the slope. It stops at the sea, at the edge or after its last step. 0 drops give the
/// same terrain.
pub fn erode(terrain: &Terrain, seed: Seed, drops: f64) -> Result<Terrain, String> {
    if !(0.0..=MAX_DROPS).contains(&drops) {
        return Err(format!("erosion must be from 0 to {MAX_DROPS} drops for each sample"));
    }
    let mut out = terrain.clone();
    let (nx, ny) = (terrain.nx, terrain.ny);
    if nx < 3 || ny < 3 || terrain.heights.len() != nx * ny || !terrain.cell.is_finite() || terrain.cell <= 0.0 {
        return Ok(out);
    }
    let (high_x, high_y) = ((nx - 1) as f64, (ny - 1) as f64);
    let brush = brush();
    let mut random = seed.random();
    for _ in 0..(drops * (nx * ny) as f64).round() as usize {
        let mut p = [random.range(0.0, high_x), random.range(0.0, high_y)];
        let (mut dir, mut speed, mut water, mut soil) = ([0.0, 0.0], 1.0, 1.0, 0.0);
        for _ in 0..STEPS {
            let (h, g) = surface(&out, p);
            dir = [0, 1].map(|k| dir[k] * INERTIA - g[k] * (1.0 - INERTIA));
            let length = dir[0].hypot(dir[1]);
            if length < 1e-9 {
                break;
            }
            dir = dir.map(|d| d / length);
            let next = [p[0] + dir[0], p[1] + dir[1]];
            let out_of_area = !(0.0..=high_x).contains(&next[0]) || !(0.0..=high_y).contains(&next[1]);
            let fall = if out_of_area { 0.0 } else { h - surface(&out, next).0 };
            if out_of_area || h - fall < terrain.sea {
                // The drop leaves the area, or the sea carries its soil away.
                soil = 0.0;
                break;
            }
            let capacity = fall.max(MIN_FALL) * speed * water * CAPACITY;
            if fall < 0.0 || soil > capacity {
                // Uphill, the drop fills the pit behind it up to the next height.
                let amount = if fall < 0.0 { soil.min(-fall) } else { (soil - capacity) * DEPOSIT };
                soil -= amount;
                add(&mut out, p, amount);
            } else {
                soil += take(&mut out, p, ((capacity - soil) * ERODE).min(fall), &brush);
            }
            speed = (speed * speed + fall / terrain.cell * GRAVITY).max(0.0).sqrt();
            water *= 1.0 - EVAPORATE;
            p = next;
        }
        add(&mut out, p, soil);
    }
    Ok(out)
}

/// The height and the rise for each sample along X and Y at a point in samples, on the bilinear
/// surface of the four samples around it.
fn surface(t: &Terrain, p: [f64; 2]) -> (f64, [f64; 2]) {
    let (i, j, u, v) = cell(t, p);
    let at = |a: usize, b: usize| f64::from(t.heights[b * t.nx + a]);
    let (h00, h10, h01, h11) = (at(i, j), at(i + 1, j), at(i, j + 1), at(i + 1, j + 1));
    let h = h00 * (1.0 - u) * (1.0 - v) + h10 * u * (1.0 - v) + h01 * (1.0 - u) * v + h11 * u * v;
    (h, [(h10 - h00) * (1.0 - v) + (h11 - h01) * v, (h01 - h00) * (1.0 - u) + (h11 - h10) * u])
}

/// The low corner of the grid cell of a point in samples, and the place of the point in it.
fn cell(t: &Terrain, p: [f64; 2]) -> (usize, usize, f64, f64) {
    let (i, j) = ((p[0] as usize).min(t.nx - 2), (p[1] as usize).min(t.ny - 2));
    (i, j, p[0] - i as f64, p[1] - j as f64)
}

/// Puts `amount` meters of soil on the four samples around a point.
fn add(t: &mut Terrain, p: [f64; 2], amount: f64) {
    let (i, j, u, v) = cell(t, p);
    let nx = t.nx;
    let corners = [(i, j, (1.0 - u) * (1.0 - v)), (i + 1, j, u * (1.0 - v)), (i, j + 1, (1.0 - u) * v), (i + 1, j + 1, u * v)];
    for (a, b, w) in corners {
        t.heights[b * nx + a] += (amount * w) as f32;
    }
}

/// Takes up to `amount` meters of soil from the samples near a point, and gives the soil taken.
/// No sample goes below the sea, so the erosion makes no new water.
fn take(t: &mut Terrain, p: [f64; 2], amount: f64, brush: &[(i64, i64, f64)]) -> f64 {
    let (ci, cj) = (p[0].round() as i64, p[1].round() as i64);
    let mut taken = 0.0;
    for &(di, dj, w) in brush {
        let (a, b) = (ci + di, cj + dj);
        if (0..t.nx as i64).contains(&a) && (0..t.ny as i64).contains(&b) {
            let h = &mut t.heights[b as usize * t.nx + a as usize];
            let cut = (amount * w).min(f64::from(*h) - t.sea).max(0.0);
            *h -= cut as f32;
            taken += cut;
        }
    }
    taken
}

/// The samples within RADIUS of a sample, with weights that fall to 0 at RADIUS and add up to 1.
fn brush() -> Vec<(i64, i64, f64)> {
    let mut brush = Vec::new();
    for dj in -RADIUS..=RADIUS {
        for di in -RADIUS..=RADIUS {
            let w = RADIUS as f64 - ((di * di + dj * dj) as f64).sqrt();
            if w > 0.0 {
                brush.push((di, dj, w));
            }
        }
    }
    let total: f64 = brush.iter().map(|b| b.2).sum();
    brush.iter().map(|&(di, dj, w)| (di, dj, w / total)).collect()
}

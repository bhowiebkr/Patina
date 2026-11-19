//! The density of the population on the terrain (task E10). The road growth, the zones and the
//! heights read it.

use std::fmt;

use geometry::Seed;

use crate::terrain::{fbm, interpolate};
use crate::{Terrain, length, sub};

/// The most centers of a density map.
pub const MAX_CENTERS: u32 = 16;

/// A value from 0 to 1 on the grid of a terrain, and the centers of the city.
#[derive(Clone, Default, PartialEq)]
pub struct Density {
    pub nx: usize,
    pub ny: usize,
    pub cell: f64,
    /// The values, row by row from the lowest Y.
    pub values: Vec<f32>,
    /// The main center first, then the sub-centers.
    pub centers: Vec<[f64; 2]>,
}

/// Shows "201 x 151 samples, 3 centers, mean 0.21, peak 0.98".
impl fmt::Debug for Density {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mean = self.values.iter().map(|&v| f64::from(v)).sum::<f64>() / self.values.len().max(1) as f64;
        let peak = self.values.iter().fold(0.0_f32, |a, &v| a.max(v));
        write!(f, "{} x {} samples, {} centers, mean {mean:.2}, peak {peak:.2}", self.nx, self.ny, self.centers.len())
    }
}

impl patina_sdk::Viewer for Density {
    fn text(&self, out: &mut patina_sdk::Text<'_>) -> fmt::Result {
        fmt::Write::write_fmt(out, format_args!("{self:?}"))
    }
}

impl Density {
    /// The density at a point. A map with no samples gives 0.
    pub fn at(&self, p: [f64; 2]) -> f64 {
        interpolate(self.nx, self.ny, self.cell, &self.values, p)
    }
}

/// A density map on the grid of the terrain. It is 1 at the main center, which is on flat land
/// near the middle, and goes down over `radius`. The sub-centers are smaller peaks. Steep
/// slopes get less, the water gets nothing, and a noise makes districts.
pub fn density(terrain: &Terrain, seed: Seed, centers: u32, radius: f64) -> Result<Density, String> {
    if !(1..=MAX_CENTERS).contains(&centers) || !(radius.is_finite() && radius > 0.0) {
        return Err(format!("centers must be from 1 to {MAX_CENTERS} and the radius more than 0"));
    }
    let t = terrain;
    if t.nx < 2 || t.ny < 2 || t.heights.len() != t.nx * t.ny {
        return Err("the terrain has no samples".into());
    }
    let mut random = seed.child("centers").random();
    // The candidates are flat land points on a coarse lattice.
    let step = t.cell.max(t.width().max(t.depth()) / 48.0);
    let (cx, cy) = ((t.width() / step) as usize, (t.depth() / step) as usize);
    let candidates: Vec<[f64; 2]> = (0..=cy)
        .flat_map(|j| (0..=cx).map(move |i| [i as f64 * step - t.width() / 2.0, j as f64 * step - t.depth() / 2.0]))
        .filter(|&p| !t.water(p) && t.slope(p) < 0.2)
        .collect();
    if candidates.is_empty() {
        return Err("the terrain has no flat land for a center".into());
    }
    let shift = [random.range(-0.3, 0.3) * radius, random.range(-0.3, 0.3) * radius];
    let main = candidates.iter().copied().min_by(|a, b| length(sub(*a, shift)).total_cmp(&length(sub(*b, shift)))).unwrap_or_default();
    let mut peaks = vec![(main, 1.0, radius)];
    for _ in 0..200 {
        if peaks.len() >= centers as usize {
            break;
        }
        let p = candidates[random.index(candidates.len())];
        if peaks.iter().all(|&(q, _, _)| length(sub(p, q)) >= 0.8 * radius) {
            peaks.push((p, random.range(0.45, 0.7), radius * random.range(0.35, 0.55)));
        }
    }
    let noise = seed.child("districts").0;
    let mut values = Vec::with_capacity(t.nx * t.ny);
    for j in 0..t.ny {
        for i in 0..t.nx {
            let p = t.point(i, j);
            if t.water(p) {
                values.push(0.0);
                continue;
            }
            let peak = peaks.iter().map(|&(q, w, r)| w * (-(length(sub(p, q)) / r).powi(2)).exp()).fold(0.0, f64::max);
            let flat = (1.0 - t.slope(p) / 0.4).clamp(0.0, 1.0);
            let district = 0.8 + 0.4 * fbm(noise, p[0] / (0.5 * radius), p[1] / (0.5 * radius), 3);
            values.push((peak * flat * district).clamp(0.0, 1.0) as f32);
        }
    }
    let centers = peaks.into_iter().map(|(p, _, _)| p).collect();
    Ok(Density { nx: t.nx, ny: t.ny, cell: t.cell, values, centers })
}

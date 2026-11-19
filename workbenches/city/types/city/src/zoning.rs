//! The zone and the floors of each lot (task E11), from the density, the streets and the slope.

use geometry::Seed;

use crate::terrain::fbm;
use crate::{ARTERIAL, Density, HIGHWAY, Plots, Terrain, Zone, centroid};

/// The most floors of a building.
pub const MAX_FLOORS: u32 = 120;

/// The size of the districts of the noise, in meters.
const DISTRICT: f64 = 160.0;

/// Gives each lot a zone. A lot with no street, on a steep slope or in the water, and a random
/// part `parks` of the other lots, become parks. A high density, or a lot on an arterial or a
/// highway in a mid density, is commercial. An industrial district of the noise, which covers
/// about the part `industry` of the land, is industrial where the land is flat and the density
/// is not high. A lot on a highway in a low density is also industrial. All other lots are
/// residential.
pub fn zoning(plots: &Plots, terrain: &Terrain, density: &Density, seed: Seed, parks: f64, industry: f64) -> Result<Plots, String> {
    if !(0.0..=1.0).contains(&parks) || !(0.0..=1.0).contains(&industry) {
        return Err(format!("parks and industry must be from 0 to 1, not {parks} and {industry}"));
    }
    let noise = seed.child("districts").0;
    let mut out = plots.clone();
    for (i, plot) in out.0.iter_mut().enumerate() {
        let c = centroid(&plot.corners);
        let (d, slope, front) = (density.at(c), terrain.slope(c), plot.front());
        // The noise is near 0.5 most of the time, so its spread is made wider first.
        let district = (0.5 + 2.2 * (fbm(noise, c[0] / DISTRICT, c[1] / DISTRICT, 2) - 0.5)).clamp(0.0, 1.0);
        let chance = seed.child(&format!("zone {i}")).random().unit();
        plot.zone = match front {
            None => Zone::Park,
            _ if slope > 0.3 || terrain.water(c) || chance < parks => Zone::Park,
            _ if d > 0.62 || (front <= Some(ARTERIAL) && d > 0.35) => Zone::Commercial,
            _ if slope < 0.12 && d < 0.55 && (district < industry || (front == Some(HIGHWAY) && d < 0.3)) => Zone::Industrial,
            _ => Zone::Residential,
        };
    }
    Ok(out)
}

/// Gives each lot its floors from the density and the zone, up to `high`. A residential lot
/// has houses of one to three floors at a low density and apartments at a high density. A
/// commercial lot has shops, and towers near `high` at the peak of the density. An industrial
/// lot has one or two floors, and a park none.
pub fn heights(plots: &Plots, density: &Density, seed: Seed, high: u32) -> Result<Plots, String> {
    if !(1..=MAX_FLOORS).contains(&high) {
        return Err(format!("high must be from 1 to {MAX_FLOORS}, not {high}"));
    }
    let mut out = plots.clone();
    let top = f64::from(high);
    for (i, plot) in out.0.iter_mut().enumerate() {
        let d = density.at(centroid(&plot.corners));
        let r = seed.child(&format!("height {i}")).random().unit();
        let floors = match plot.zone {
            Zone::Park => 0.0,
            Zone::Industrial => 1.0 + f64::from(u8::from(r < 0.25)),
            Zone::Residential => 1.0 + (d * d * 0.35 * top * (0.6 + 0.8 * r)).round(),
            Zone::Commercial => 2.0 + (d.powf(2.5) * top * (0.5 + r)).round(),
        };
        plot.floors = (floors as u32).min(high);
    }
    Ok(out)
}

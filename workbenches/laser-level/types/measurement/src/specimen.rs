//! Known geometry drives the synthetic sensor. It never supplies measured results.
use crate::core::{survey::Run, synth::Surface};

pub fn height(run: &Run, station: usize, object: u32, relief_um: f64) -> f64 {
    if object == 0 {
        return Surface::default().seen_um(run, station);
    }
    let p = &run.stations[station];
    coin(p.x_mm, p.y_mm) * relief_um
}

/// An authored 26 mm medallion on a 30 mm bed. Values are normalized height, not image brightness.
/// The relief has a raised rim, beads, a compass rose, a central boss and two engraved rings.
pub fn coin(x_mm: f64, y_mm: f64) -> f64 {
    let (x, y) = ((x_mm - 15.0) / 13.0, (y_mm - 15.0) / 13.0);
    let r = x.hypot(y);
    let edge = smooth((1.0 - r) / 0.025);
    if edge == 0.0 {
        return 0.0;
    }
    let a = y.atan2(x);
    let ridge = |at: f64, width: f64| (-((r - at) / width).powi(2)).exp();
    let rim = 0.20 * ridge(0.945, 0.030);
    let beads = 0.10 * ridge(0.84, 0.025) * (0.5 + 0.5 * (a * 40.0).cos()).powi(2);
    let star_edge = 0.23 + 0.42 * (0.5 + 0.5 * (a * 8.0).cos()).powi(3);
    let star = smooth((star_edge - r) / 0.025);
    let facets = (1.0 - r / star_edge.max(0.01)).clamp(0.0, 1.0);
    let rose = star * (0.12 + 0.35 * facets);
    let hub = 0.12 * (-(r / 0.105).powi(2)).exp();
    let rings = -0.055 * ridge(0.74, 0.016) - 0.04 * ridge(0.69, 0.012);
    // Four broad diamonds remain visible at the demo's sampling pitch.
    let mut diamonds = 0.0_f64;
    for (cx, cy) in [(0.0, 0.57), (0.57, 0.0), (0.0, -0.57), (-0.57, 0.0)] {
        diamonds = diamonds.max(0.16 * smooth((0.10 - (x - cx).abs() - (y - cy).abs()) / 0.04));
    }
    edge * (0.42 + rim + beads + rose + hub + rings + diamonds)
}

fn smooth(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn relief_has_a_bed_rim_and_raised_engraving() {
        assert_eq!(coin(0.0, 0.0), 0.0);
        assert!(coin(15.0, 15.0) > 0.9);
        assert!(coin(15.0 + 13.0 * 0.945, 15.0) > 0.60);
        assert!(coin(15.0 + 13.0 * 0.74, 15.0) < 0.40);
        for y in 0..121 {
            for x in 0..121 {
                assert!((0.0..=1.1).contains(&coin(x as f64 / 4.0, y as f64 / 4.0)));
            }
        }
    }
}

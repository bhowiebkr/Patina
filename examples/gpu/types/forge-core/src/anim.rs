//! Baked animation clip (the CAD pipeline's native clip format): one sample per tick, each
//! track the absolute translation, rotation and scale of one part. The clip loops.

use glam::{Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Clip {
    #[serde(default)]
    pub label: String,
    pub sample_rate: f32,
    pub duration: f32,
    #[serde(default)]
    pub cycle_ticks: u32,
    #[serde(default)]
    pub cycles: u32,
    pub tracks: Vec<Track>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Track {
    pub part: u32,
    /// xyz per sample.
    pub positions: Vec<f32>,
    /// xyzw per sample.
    pub quaternions: Vec<f32>,
    /// xyz per sample.
    pub scales: Vec<f32>,
}

impl Track {
    pub fn samples(&self) -> usize {
        (self.positions.len() / 3)
            .min(self.quaternions.len() / 4)
            .min(self.scales.len() / 3)
    }

    fn trs(&self, i: usize) -> (Vec3, Quat, Vec3) {
        let p = &self.positions[i * 3..i * 3 + 3];
        let q = &self.quaternions[i * 4..i * 4 + 4];
        let s = &self.scales[i * 3..i * 3 + 3];
        (
            Vec3::new(p[0], p[1], p[2]),
            Quat::from_xyzw(q[0], q[1], q[2], q[3]).normalize(),
            Vec3::new(s[0], s[1], s[2]),
        )
    }

    /// Part matrix at `t` seconds, looping, interpolated between ticks.
    pub fn sample(&self, t: f32, sample_rate: f32) -> Mat4 {
        let n = self.samples();
        if n == 0 {
            return Mat4::IDENTITY;
        }
        let f = (t * sample_rate).rem_euclid(n as f32);
        let i = (f.floor() as usize).min(n - 1);
        let k = f - i as f32;
        let (p0, q0, s0) = self.trs(i);
        let (p1, q1, s1) = self.trs((i + 1) % n);
        Mat4::from_scale_rotation_translation(s0.lerp(s1, k), q0.slerp(q1, k), p0.lerp(p1, k))
    }
}

impl Clip {
    /// Length of one loop in seconds.
    pub fn length(&self) -> f32 {
        let n = self.tracks.iter().map(Track::samples).max().unwrap_or(0);
        if n > 0 && self.sample_rate > 0.0 {
            n as f32 / self.sample_rate
        } else {
            self.duration
        }
    }

    /// Writes each animated part's matrix at `t` into `out` (indexed by part).
    pub fn pose(&self, t: f32, out: &mut [Mat4]) {
        for track in &self.tracks {
            if let Some(m) = out.get_mut(track.part as usize) {
                *m = track.sample(t, self.sample_rate);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_interpolate_and_loop() {
        let track = Track {
            part: 0,
            positions: vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            quaternions: vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            scales: vec![1.0; 6],
        };
        let at = |t| track.sample(t, 1.0).w_axis.x;
        assert!((at(0.5) - 0.5).abs() < 1e-6);
        assert!(
            (at(1.5) - 0.5).abs() < 1e-6,
            "wraps from last to first sample"
        );
        assert!((at(2.0) - 0.0).abs() < 1e-6);
    }
}

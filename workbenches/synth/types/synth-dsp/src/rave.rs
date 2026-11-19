//! A rough root-and-fifth source for distorted, short rave leads.
use crate::{RATE, finite, voice::wave};
use serde::{Deserialize, Serialize};
mod decimator;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RaveTone {
    pub fifth: f32,
    pub body: f32,
    pub bits: u8,
}

impl Default for RaveTone {
    fn default() -> Self {
        Self {
            fifth: 0.58,
            body: 1500.0,
            bits: 8,
        }
    }
}

impl RaveTone {
    pub fn valid(self) -> bool {
        self.fifth.is_finite()
            && (0.0..=1.0).contains(&self.fifth)
            && self.body.is_finite()
            && (200.0..=8000.0).contains(&self.body)
            && (4..=16).contains(&self.bits)
    }
}

#[derive(Clone, Default)]
pub struct RaveOscillator {
    phase: [f64; 2],
    low: [f32; 2],
    body: f32,
    alpha: f32,
    decimator: decimator::Decimator,
}

const FIFTH: f64 = 1.498_307_076_876_681_5;

impl RaveOscillator {
    pub fn reset(&mut self, hz: f32, elapsed: u64) {
        let hz = finite(hz, 440.0, 10.0, 12000.0);
        let cycles = elapsed as f64 * f64::from(hz) / f64::from(RATE);
        self.phase = [cycles.fract(), (cycles * FIFTH).fract()];
        self.low = [0.0; 2];
        self.decimator = Default::default();
    }

    pub fn next(&mut self, hz: f32, tone: RaveTone) -> f32 {
        self.next_driven(hz, tone, 1.0)
    }

    pub fn next_driven(&mut self, hz: f32, tone: RaveTone, drive: f32) -> f32 {
        let dt = f64::from(finite(hz, 440.0, 10.0, 12000.0)) / (4.0 * f64::from(RATE));
        let drive = finite(drive, 1.0, 1.0, 16.0);
        let fifth = finite(tone.fifth, 0.58, 0.0, 1.0);
        let body = finite(tone.body, 1500.0, 200.0, 8000.0);
        if self.body != body {
            self.body = body;
            self.alpha = 1.0 - (-std::f32::consts::TAU * body / (4.0 * RATE)).exp();
        }
        let levels = ((1_u32 << (tone.bits.clamp(4, 16) - 1)) - 1) as f32;
        for _ in 0..4 {
            let root = wave(self.phase[0] as f32, dt as f32, 1);
            let upper = wave(self.phase[1] as f32, (dt * FIFTH) as f32, 2);
            let source = (root + upper * fifth) / (1.0 + fifth);
            self.low[0] += self.alpha * (source - self.low[0]);
            self.low[1] += self.alpha * (self.low[0] - self.low[1]);
            for (phase, ratio) in self.phase.iter_mut().zip([1.0, FIFTH]) {
                *phase = (*phase + dt * ratio).fract();
            }
            let source = (self.low[1] * levels).round() / levels;
            let x = (source * drive).clamp(-3.0, 3.0);
            let shaped = if drive == 1.0 {
                source
            } else {
                x * (27.0 + x * x) / (27.0 + 9.0 * x * x)
            };
            self.decimator.push(shaped * 0.85);
        }
        self.decimator.read()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oversampled_drive_reduces_the_folded_fifth_harmonic() {
        let tone = RaveTone {
            fifth: 0.0,
            body: 8000.0,
            bits: 16,
        };
        let mut source = RaveOscillator::default();
        let mut phase = 0.0_f32;
        let mut low = [0.0; 2];
        let alpha = 1.0 - (-std::f32::consts::TAU * tone.body / RATE).exp();
        let mut projections = [[0.0_f64; 4]; 2];
        for i in 0..9600 {
            let oversampled = source.next_driven(7000.0, tone, 16.0);
            let raw = wave(phase, 7000.0 / RATE, 1);
            phase = (phase + 7000.0 / RATE).fract();
            low[0] += alpha * (raw - low[0]);
            low[1] += alpha * (low[0] - low[1]);
            let x = (low[1] * 16.0).clamp(-3.0, 3.0);
            let naive = x * (27.0 + x * x) / (27.0 + 9.0 * x * x) * 0.85;
            if i >= 4800 {
                for (bins, value) in projections.iter_mut().zip([oversampled, naive]) {
                    for (j, hz) in [7000.0, 13000.0].iter().enumerate() {
                        let p = std::f64::consts::TAU * hz * i as f64 / 48000.0;
                        bins[j * 2] += f64::from(value) * p.cos();
                        bins[j * 2 + 1] += f64::from(value) * p.sin();
                    }
                }
            }
        }
        let ratios = projections.map(|p| p[2].hypot(p[3]) / p[0].hypot(p[1]));
        assert!(
            ratios[0] < ratios[1] * 0.2,
            "folded/fundamental: {ratios:?}"
        );
    }

    #[test]
    fn malformed_node_inputs_remain_finite() {
        let mut source = RaveOscillator::default();
        source.reset(f32::NAN, 12);
        let tone = RaveTone {
            fifth: f32::INFINITY,
            body: f32::NAN,
            bits: 255,
        };
        for _ in 0..256 {
            assert!(source.next(f32::INFINITY, tone).is_finite());
        }
    }

    #[test]
    fn fifth_and_bit_depth_change_the_source_without_allocations() {
        let mut a = RaveOscillator::default();
        let mut b = RaveOscillator::default();
        let mut c = RaveOscillator::default();
        let mut difference = [0.0; 2];
        let mut peak = 0.0_f32;
        let tone = RaveTone::default();
        let start = std::time::Instant::now();
        crate::tests::allocations::COUNT.set(0);
        crate::tests::allocations::ENABLED.set(true);
        for _ in 0..4800 {
            let raw = a.next(246.94165, tone);
            let root = b.next(246.94165, RaveTone { fifth: 0.0, ..tone });
            let rough = c.next(246.94165, RaveTone { bits: 4, ..tone });
            assert!(raw.is_finite() && raw.abs() <= 0.85);
            difference[0] += (raw - root).abs();
            difference[1] += (raw - rough).abs();
            peak = peak.max(raw.abs());
        }
        crate::tests::allocations::ENABLED.set(false);
        assert_eq!(crate::tests::allocations::COUNT.get(), 0);
        assert!(difference[0] > 100.0 && difference[1] > 10.0);
        assert!(peak > 0.2);
        println!(
            "Rave source: {:.3} ms per 256 samples, zero allocations (three source settings)",
            start.elapsed().as_secs_f64() * 1000.0 * 256.0 / (4800.0 * 3.0)
        );
    }
}

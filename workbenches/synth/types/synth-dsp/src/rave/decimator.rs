//! Four-times decimation after the nonlinear source, with a 96-tap Blackman FIR.
#[derive(Clone)]
pub(super) struct Decimator {
    history: [f32; 96],
    write: usize,
}
impl Default for Decimator {
    fn default() -> Self {
        Self {
            history: [0.0; 96],
            write: 0,
        }
    }
}
impl Decimator {
    pub fn push(&mut self, value: f32) {
        self.history[self.write] = value;
        self.write = (self.write + 1) % self.history.len();
    }
    pub fn read(&self) -> f32 {
        TAPS.iter()
            .enumerate()
            .map(|(i, tap)| self.history[(self.write + 95 - i) % 96] * tap)
            .sum()
    }
}
// 192 kHz input, 20.16 kHz cutoff, unity DC gain.
const TAPS: [f32; 96] = [
    0.000000,
    -0.000001817,
    -0.000010921,
    -0.000022714,
    -0.00001944,
    0.000017873,
    0.000089512,
    0.000162838,
    0.000177891,
    0.000077863,
    -0.000146392,
    -0.000422893,
    -0.00060414,
    -0.000525495,
    -0.000101414,
    0.000585364,
    0.001259729,
    0.00153559,
    0.001094713,
    -0.000105626,
    -0.001705612,
    -0.002996823,
    -0.003187876,
    -0.001803852,
    0.000956698,
    0.004113337,
    0.006176263,
    0.005766368,
    0.002356683,
    -0.003215218,
    -0.008787989,
    -0.01159519,
    -0.009476105,
    -0.002103067,
    0.008370276,
    0.01769206,
    0.02098878,
    0.01490084,
    -0.000446929,
    -0.02054734,
    -0.03728466,
    -0.04135961,
    -0.02564579,
    0.01172643,
    0.0658004,
    0.1255114,
    0.1766433,
    0.2061127,
    0.2061127,
    0.1766433,
    0.1255114,
    0.0658004,
    0.01172643,
    -0.02564579,
    -0.04135961,
    -0.03728466,
    -0.02054734,
    -0.000446929,
    0.01490084,
    0.02098878,
    0.01769206,
    0.008370276,
    -0.002103067,
    -0.009476105,
    -0.01159519,
    -0.008787989,
    -0.003215218,
    0.002356683,
    0.005766368,
    0.006176263,
    0.004113337,
    0.000956698,
    -0.001803852,
    -0.003187876,
    -0.002996823,
    -0.001705612,
    -0.000105626,
    0.001094713,
    0.00153559,
    0.001259729,
    0.000585364,
    -0.000101414,
    -0.000525495,
    -0.00060414,
    -0.000422893,
    -0.000146392,
    0.000077863,
    0.000177891,
    0.000162838,
    0.000089512,
    0.000017873,
    -0.00001944,
    -0.000022714,
    -0.000010921,
    -0.000001817,
    0.000000,
];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decimator_preserves_the_passband_and_rejects_ultrasonic_energy() {
        let response = |hz: f32| {
            let mut real = 0.0;
            let mut imag = 0.0;
            for (i, tap) in TAPS.iter().enumerate() {
                let phase = std::f32::consts::TAU * hz * i as f32 / 192000.0;
                real += tap * phase.cos();
                imag += tap * phase.sin();
            }
            (real * real + imag * imag).sqrt()
        };
        assert!((response(0.0) - 1.0).abs() < 1e-6);
        assert!(response(12000.0) > 0.99);
        assert!(response(28000.0) < 0.001);
        assert!(response(48000.0) < 0.001);
    }
}

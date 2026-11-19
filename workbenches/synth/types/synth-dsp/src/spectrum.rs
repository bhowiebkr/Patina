use std::f32::consts::TAU;

pub struct Spectrum {
    values: Vec<[f32; 2]>,
    pub bands: [f32; 96],
}
impl Default for Spectrum {
    fn default() -> Self {
        Self {
            values: vec![[0.0; 2]; 8192],
            bands: [-100.0; 96],
        }
    }
}
impl Spectrum {
    pub fn update(&mut self, samples: &[f32]) {
        self.analyze(samples);
        let n = samples.len();
        for (i, band) in self.bands.iter_mut().enumerate() {
            let hz = 20.0 * 1200.0_f32.powf(i as f32 / 95.0);
            let bin = ((hz * n as f32 / 48000.0).round() as usize).clamp(1, n / 2);
            let [re, im] = self.values[bin];
            *band = db((re * re + im * im).sqrt() * 4.0 / n as f32);
        }
    }

    pub(crate) fn analyze(&mut self, samples: &[f32]) {
        let n = samples.len();
        assert!(n.is_power_of_two() && (256..=8192).contains(&n));
        let bits = n.trailing_zeros();
        for (i, sample) in samples.iter().enumerate() {
            let reversed = i.reverse_bits() >> (usize::BITS - bits);
            self.values[reversed] = [
                sample * (0.5 - 0.5 * (TAU * i as f32 / n as f32).cos()),
                0.0,
            ];
        }
        let mut length = 2;
        while length <= n {
            let angle = -TAU / length as f32;
            let step = [angle.cos(), angle.sin()];
            for start in (0..n).step_by(length) {
                let mut phase = [1.0, 0.0];
                for j in 0..length / 2 {
                    let a = self.values[start + j];
                    let b = self.values[start + j + length / 2];
                    let b = [
                        b[0] * phase[0] - b[1] * phase[1],
                        b[0] * phase[1] + b[1] * phase[0],
                    ];
                    self.values[start + j] = [a[0] + b[0], a[1] + b[1]];
                    self.values[start + j + length / 2] = [a[0] - b[0], a[1] - b[1]];
                    phase = [
                        phase[0] * step[0] - phase[1] * step[1],
                        phase[0] * step[1] + phase[1] * step[0],
                    ];
                }
            }
            length *= 2;
        }
    }
    pub(crate) fn bin_db(&self, bin: usize, length: usize) -> f32 {
        let [re, im] = self.values[bin.min(length / 2)];
        let scale = if bin == 0 || bin == length / 2 {
            2.0
        } else {
            4.0
        };
        db(re.hypot(im) * scale / length as f32)
    }
}
pub fn db(value: f32) -> f32 {
    20.0 * value.max(0.00001).log10()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bass_spectrum_has_correct_frequency_and_level() {
        let n = 8192;
        let hz = 48000.0 * 10.0 / n as f32;
        let samples: Vec<_> = (0..n)
            .map(|i| 0.5 * (TAU * i as f32 * hz / 48000.0).sin())
            .collect();
        let mut s = Spectrum::default();
        s.update(&samples);
        let (i, level) = s
            .bands
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .unwrap();
        let peak = 20.0 * 1200.0_f32.powf(i as f32 / 95.0);
        assert!((peak - hz).abs() < 4.0, "{peak} vs {hz}");
        assert!((*level + 6.0206).abs() < 0.1, "{level}");
    }
}

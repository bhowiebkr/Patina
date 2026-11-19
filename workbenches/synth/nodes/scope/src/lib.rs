use panel::{INK, MUTED, TEAL, VIOLET, panel, screen, trace};
use patina_sdk::{AudioBlock, Ui, audio::BLOCK};
use std::f32::consts::TAU;

const SIZE: [f32; 2] = [416.0, 300.0];

/// Measures the real output. The upper trace shows 5.33 ms. The lower bars show a windowed spectrum of one block.
#[patina_sdk::node(domain = frame, free_layout, full_gui, color = "#59366f")]
pub struct Scope {
    samples: [f32; 128],
    spectrum: [f32; 48],
    peak: f32,
    rms: f32,
    weights: Box<[[[f32; 2]; BLOCK]; 48]>,
}
impl Scope {
    pub fn init() -> Self {
        let weights = Box::new(std::array::from_fn(|band| {
            let bin = 128.0_f32.powf(band as f32 / 47.0);
            std::array::from_fn(|i| {
                let phase = TAU * i as f32 / BLOCK as f32;
                let window = 0.5 - 0.5 * phase.cos();
                [window * (phase * bin).cos(), -window * (phase * bin).sin()]
            })
        }));
        Self {
            samples: [0.0; 128],
            spectrum: [0.0; 48],
            peak: 0.0,
            rms: 0.0,
            weights,
        }
    }
    pub fn process(&mut self, sound: &AudioBlock) -> f32 {
        for (i, s) in self.samples.iter_mut().enumerate() {
            *s = sound[i * 2];
        }
        self.peak = sound.peak();
        self.rms = (sound.iter().map(|s| s * s).sum::<f32>() / BLOCK as f32).sqrt();
        for (band, level) in self.spectrum.iter_mut().enumerate() {
            let (mut re, mut im) = (0.0, 0.0);
            for (i, s) in sound.iter().enumerate() {
                let [cos, sin] = self.weights[band][i];
                re += s * cos;
                im += s * sin;
            }
            let db = 20.0
                * ((re * re + im * im).sqrt() * 4.0 / BLOCK as f32)
                    .max(0.00001)
                    .log10();
            *level = (*level * 0.7).max(((db + 72.0) / 72.0).clamp(0.0, 1.0));
        }
        self.peak
    }
    pub fn ui(&mut self, ui: &mut Ui) {
        let mut c = ui.canvas("Output waveform and spectrum", SIZE);
        panel(
            &mut c,
            SIZE,
            "SIGNAL / OBSERVATORY",
            "06   SEE WHAT YOU HEAR",
            VIOLET,
        );
        screen(&mut c, [16.0, 72.0, 384.0, 80.0]);
        trace(&mut c, [20.0, 76.0, 376.0, 72.0], &self.samples, TEAL);
        c.text([20.0, 168.0], MUTED, "0 ms");
        c.text([350.0, 168.0], MUTED, "5.33 ms");
        screen(&mut c, [16.0, 180.0, 384.0, 66.0]);
        for (i, value) in self.spectrum.iter().enumerate() {
            let h = value * 58.0;
            c.rect([20.0 + i as f32 * 7.8, 242.0 - h], [5.0, h], TEAL, true);
        }
        c.text([20.0, 262.0], MUTED, "188 Hz");
        c.text([148.0, 262.0], MUTED, "1k");
        c.text([324.0, 262.0], MUTED, "10k   24k");
        c.text(
            [20.0, 285.0],
            INK,
            &format!("PEAK {:.1} dBFS", db(self.peak)),
        );
        c.text(
            [224.0, 285.0],
            INK,
            &format!("RMS {:.1} dBFS", db(self.rms)),
        );
        drop(c);
        ui.pin("sound", [0.0, 112.0]);
    }
}
fn db(value: f32) -> f32 {
    20.0 * value.max(0.00001).log10()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_scope_measures_the_input_and_finds_its_fundamental() {
        let mut scope = Scope::init();
        let sound = AudioBlock::from_fn(|i| 0.5 * (TAU * i as f32 * 8.0 / BLOCK as f32).sin());
        scope.process(&sound);
        assert!((scope.peak - 0.5).abs() < 0.001);
        assert!((scope.rms - 0.5 / 2.0_f32.sqrt()).abs() < 0.001);
        let band = scope
            .spectrum
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .unwrap()
            .0;
        assert!((19..=21).contains(&band), "peak band {band}");
    }
}

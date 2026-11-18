use patina_sdk::{AudioBlock, audio};

/// Mixes the synth and drums, limits peaks and plays the result. **Audible** controls the device output.
#[patina_sdk::node(domain = audio, color = "#59366f")]
pub struct Output {
    gain: f32,
}
impl Output {
    pub fn init() -> Self {
        Self { gain: 0.0 }
    }
    pub fn process(
        &mut self,
        synth: &AudioBlock,
        drums: &AudioBlock,
        level: f32,
        rhythm: f32,
        audible: bool,
    ) -> AudioBlock {
        let level = if level.is_finite() {
            level.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let rhythm = if rhythm.is_finite() {
            rhythm.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let block = AudioBlock::from_fn(|i| {
            self.gain += (level - self.gain) * 0.002;
            ((synth[i] + drums[i] * rhythm) * self.gain).tanh() * 0.7
        });
        if audible {
            audio::play(&block);
        }
        block
    }
}

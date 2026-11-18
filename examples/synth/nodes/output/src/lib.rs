use patina_sdk::{AudioBlock, audio};

/// Crossfades the workbench patches, limits peaks, and plays the result. Audible controls the device output.
#[patina_sdk::node(domain = audio, color = "#59366f")]
pub struct Output {
    gain: f32,
    scene: [f32; 3],
}
impl Output {
    pub fn init() -> Self {
        Self {
            gain: 0.0,
            scene: [1.0, 0.0, 0.0],
        }
    }
    #[allow(clippy::too_many_arguments)] // Each argument is an editable graph input.
    pub fn process(
        &mut self,
        synth: &AudioBlock,
        drums: &AudioBlock,
        ambient: &AudioBlock,
        impact: &AudioBlock,
        level: f32,
        rhythm: f32,
        scene: u8,
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
        let selected = usize::from(scene.min(2));
        let block = AudioBlock::from_fn(|i| {
            self.gain += (level - self.gain) * 0.002;
            for (n, amount) in self.scene.iter_mut().enumerate() {
                *amount += (f32::from(n == selected) - *amount) * 0.002;
            }
            let value = (synth[i] + drums[i] * rhythm) * self.scene[0]
                + ambient[i] * self.scene[1]
                + impact[i] * self.scene[2];
            if value.is_finite() {
                (value * self.gain).tanh() * 0.7
            } else {
                0.0
            }
        });
        if audible {
            audio::play(&block);
        }
        block
    }
}

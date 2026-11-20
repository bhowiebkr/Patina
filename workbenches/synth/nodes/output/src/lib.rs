use panel::{Ctl, Live, Look, Strip};
use patina_sdk::{AudioBlock, audio};

/// Crossfades the workbench patches, limits peaks, and plays the result. Audible controls the device output.
#[patina_sdk::node(domain = audio, free_layout, color = "#59366f")]
pub struct Output {
    live: Live,
    capture: synth_dsp::capture::Capture,
    audition: synth_dsp::audition::Audition,
    gain: f32,
    scene: [f32; 3],
}
impl Output {
    pub fn init() -> Self {
        Self {
            live: Live::default(),
            capture: synth_dsp::capture::Capture::new("Output"),
            audition: Default::default(),
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
        let mut block = AudioBlock::from_fn(|i| {
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
        self.audition.mix(&mut block);
        self.capture.push(&block);
        if audible {
            audio::play(&block);
        }
        self.live.keep(
            &block,
            &block,
            [f32::from(u8::from(audible)), 0.0, 0.0, 0.0, 0.0],
        );
        block
    }
    pub fn display(&self) -> Live {
        self.live
    }
    pub fn ui(gui: &mut Strip, shown: &Live, ui: &mut patina_sdk::Ui) {
        let look = Look {
            title: "OUTPUT",
            subtitle: "SCENE MIX",
            color: panel::VIOLET,
        };
        gui.show(ui, shown, look, &[Ctl::switch("audible", "AUDIBLE")]);
    }
}

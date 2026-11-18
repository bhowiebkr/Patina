use panel::{Dial, Knob, MUTED, VIOLET, inspector, panel, screen};
use patina_sdk::Ui;
use synth_dsp::{Space, finite};

const SIZE: [f32; 2] = [300.0, 272.0];

/// **Time** sets echo spacing. **Feedback** returns each echo to the delay. **Wet** sets its level.
#[patina_sdk::node(domain = frame, free_layout, full_gui, color = "#59366f")]
pub struct EchoControls {
    space: Space,
    dials: [Dial; 3],
}
impl EchoControls {
    pub fn init() -> Self {
        Self {
            space: Space::default(),
            dials: Default::default(),
        }
    }
    pub fn process(&mut self, seconds: f32, feedback: f32, mix: f32) -> Space {
        self.space = Space {
            seconds: finite(seconds, 0.402, 0.02, 1.5),
            feedback: finite(feedback, 0.42, 0.0, 0.85),
            mix: finite(mix, 0.3, 0.0, 0.8),
        };
        self.space
    }
    pub fn ui(&mut self, ui: &mut Ui) {
        if inspector(
            ui,
            &[
                "Time sets the gap between echoes.",
                "Feedback returns sound to the delay.",
                "Wet sets the amount of echo.",
            ],
        ) {
            return;
        }
        {
            let mut c = ui.canvas("Echo time feedback and wet mix", SIZE);
            panel(
                &mut c,
                SIZE,
                "TAPE ECHO",
                "05   GIVE THE PHRASE SPACE",
                VIOLET,
            );
            screen(&mut c, [16.0, 72.0, 268.0, 60.0]);
            for i in 0..8 {
                let x = 24.0 + i as f32 * self.space.seconds * 70.0;
                if x < 278.0 {
                    let h = 44.0
                        * self.space.feedback.powi(i)
                        * if i == 0 { 1.0 } else { self.space.mix };
                    c.rect([x, 124.0 - h], [5.0, h], VIOLET, true);
                }
            }
            c.text([18.0, 148.0], MUTED, "DIRECT");
            c.text([165.0, 148.0], MUTED, "REPEATS / 3.6 s");
        }
        self.dials[0].show(
            ui,
            &mut self.space.seconds,
            Knob {
                at: [10.0, 154.0],
                key: "seconds",
                label: "TIME",
                range: [0.02, 1.5],
                color: VIOLET,
                logarithmic: true,
                unit: "s",
            },
        );
        self.dials[1].show(
            ui,
            &mut self.space.feedback,
            Knob {
                at: [106.0, 154.0],
                key: "feedback",
                label: "FEEDBACK",
                range: [0.0, 0.85],
                color: VIOLET,
                logarithmic: false,
                unit: "",
            },
        );
        self.dials[2].show(
            ui,
            &mut self.space.mix,
            Knob {
                at: [202.0, 154.0],
                key: "mix",
                label: "WET",
                range: [0.0, 0.8],
                color: VIOLET,
                logarithmic: false,
                unit: "",
            },
        );
        ui.pin("out", [300.0, 136.0]);
    }
}

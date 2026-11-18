use panel::{AMBER, Dial, Knob, MUTED, button, inside, inspector, panel, screen, trace};
use patina_sdk::{Pointer, Ui};
use synth_dsp::{Tone, finite};

const SIZE: [f32; 2] = [324.0, 320.0];

/// **Shape** sets the harmonics. **Detune** separates two pitches. **Sub** adds a sine one octave below.
#[patina_sdk::node(domain = frame, free_layout, full_gui, color = "#655219")]
pub struct Timbre {
    tone: Tone,
    dials: [Dial; 2],
}
impl Timbre {
    pub fn init() -> Self {
        Self {
            tone: Tone::default(),
            dials: Default::default(),
        }
    }
    pub fn process(&mut self, shape: u8, detune: f32, sub: f32, octave: i8) -> Tone {
        self.tone = Tone {
            shape: shape.min(3),
            detune: finite(detune, 9.0, 0.0, 30.0),
            sub: finite(sub, 0.28, 0.0, 1.0),
            octave: octave.clamp(-2, 2),
        };
        self.tone
    }
    pub fn ui(&mut self, ui: &mut Ui) {
        if inspector(
            ui,
            &[
                "Sine: soft. Saw: bright. Pulse: hollow.",
                "Triangle has fewer upper harmonics.",
                "Detune separates the two oscillators.",
                "Sub adds a sine one octave below.",
            ],
        ) {
            return;
        }
        let (old, octave) = (self.tone.shape, self.tone.octave);
        {
            let mut c = ui.canvas(&format!("Timbre shape {}", self.tone.shape), SIZE);
            for p in c.pointer() {
                if let Pointer::Down(at) = *p {
                    for i in 0..4 {
                        if inside(at, [16.0 + i as f32 * 74.0, 72.0, 68.0, 28.0]) {
                            self.tone.shape = i;
                        }
                    }
                    if inside(at, [220.0, 211.0, 36.0, 28.0]) {
                        self.tone.octave = (self.tone.octave - 1).max(-2);
                    }
                    if inside(at, [268.0, 211.0, 36.0, 28.0]) {
                        self.tone.octave = (self.tone.octave + 1).min(2);
                    }
                }
            }
            panel(
                &mut c,
                SIZE,
                "TWIN OSCILLATOR",
                "02   CREATE THE RAW TONE",
                AMBER,
            );
            for (i, label) in ["SINE", "SAW", "PULSE", "TRI"].iter().enumerate() {
                button(
                    &mut c,
                    [16.0 + i as f32 * 74.0, 72.0, 68.0, 28.0],
                    label,
                    self.tone.shape == i as u8,
                    AMBER,
                );
            }
            screen(&mut c, [16.0, 112.0, 292.0, 68.0]);
            let samples: [f32; 96] = std::array::from_fn(|i| {
                let t = (i as f32 / 95.0 * 2.0).fract();
                match self.tone.shape {
                    0 => (t * std::f32::consts::TAU).sin(),
                    1 => t * 2.0 - 1.0,
                    2 => {
                        if t < 0.5 {
                            1.0
                        } else {
                            -1.0
                        }
                    }
                    _ => 1.0 - 4.0 * (t - 0.5).abs(),
                }
            });
            trace(&mut c, [20.0, 117.0, 284.0, 58.0], &samples, AMBER);
            c.text([220.0, 201.0], MUTED, "OCTAVE");
            button(&mut c, [220.0, 211.0, 36.0, 28.0], "-", false, AMBER);
            button(&mut c, [268.0, 211.0, 36.0, 28.0], "+", false, AMBER);
            c.text([243.0, 263.0], AMBER, &format!("{:+}", self.tone.octave));
        }
        if old != self.tone.shape {
            ui.set_param("shape", self.tone.shape);
        }
        if octave != self.tone.octave {
            ui.set_param("octave", self.tone.octave);
        }
        self.dials[0].show(
            ui,
            &mut self.tone.detune,
            Knob {
                at: [16.0, 192.0],
                key: "detune",
                label: "DETUNE",
                range: [0.0, 30.0],
                color: AMBER,
                logarithmic: false,
                unit: "ct",
            },
        );
        self.dials[1].show(
            ui,
            &mut self.tone.sub,
            Knob {
                at: [116.0, 192.0],
                key: "sub",
                label: "SUB",
                range: [0.0, 1.0],
                color: AMBER,
                logarithmic: false,
                unit: "",
            },
        );
        ui.pin("out", [324.0, 160.0]);
    }
}

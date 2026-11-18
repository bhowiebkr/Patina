use panel::{Dial, Knob, MUTED, TEAL, inspector, panel, screen};
use patina_sdk::Ui;
use std::fmt::Write;
use synth_dsp::{Color, finite};

const SIZE: [f32; 2] = [284.0, 392.0];

/// **Cutoff** removes high harmonics. **Resonance** adds emphasis. An LFO moves the cutoff through **Motion** octaves.
#[patina_sdk::node(domain = frame, free_layout, full_gui, color = "#175c59")]
pub struct FilterControls {
    color: Color,
    dials: [Dial; 4],
}
impl FilterControls {
    pub fn init() -> Self {
        Self {
            color: Color::default(),
            dials: Default::default(),
        }
    }
    pub fn process(&mut self, cutoff: f32, resonance: f32, motion: f32, rate: f32) -> Color {
        self.color = Color {
            cutoff: finite(cutoff, 1600.0, 60.0, 16000.0),
            resonance: finite(resonance, 0.4, 0.0, 0.95),
            motion: finite(motion, 1.2, 0.0, 4.0),
            rate: finite(rate, 0.18, 0.02, 8.0),
        };
        self.color
    }
    pub fn ui(&mut self, ui: &mut Ui) {
        if inspector(
            ui,
            &[
                "Cutoff removes high frequencies.",
                "Resonance adds emphasis near the cutoff.",
                "Motion sets the LFO depth in octaves.",
                "LFO rate sets modulation cycles per second.",
            ],
        ) {
            return;
        }
        {
            let mut c = ui.canvas("Filter response and motion", SIZE);
            panel(
                &mut c,
                SIZE,
                "RESONANT FILTER",
                "04   SCULPT THE HARMONICS",
                TEAL,
            );
            screen(&mut c, [16.0, 72.0, 252.0, 70.0]);
            let mut path = String::new();
            let k = 2.0 - 1.9 * self.color.resonance;
            for i in 0..64 {
                let f = 20.0 * 1000.0_f32.powf(i as f32 / 63.0);
                let r = f / self.color.cutoff;
                let db = -10.0
                    * ((1.0 - r * r).powi(2) + (k * r).powi(2))
                        .max(0.0001)
                        .log10();
                let y = 90.0 - db.clamp(-36.0, 12.0) * 1.2;
                let _ = write!(
                    path,
                    "{} {:.1} {:.1} ",
                    if i == 0 { "M" } else { "L" },
                    20.0 + i as f32 * 244.0 / 63.0,
                    y
                );
            }
            c.path(&path, TEAL, false);
            c.text([18.0, 157.0], MUTED, "20 Hz");
            c.text([151.0, 157.0], MUTED, "1k");
            c.text([242.0, 157.0], MUTED, "20k");
        }
        self.dials[0].show(
            ui,
            &mut self.color.cutoff,
            Knob {
                at: [31.0, 165.0],
                key: "cutoff",
                label: "CUTOFF",
                range: [60.0, 16000.0],
                color: TEAL,
                logarithmic: true,
                unit: "Hz",
            },
        );
        self.dials[1].show(
            ui,
            &mut self.color.resonance,
            Knob {
                at: [165.0, 165.0],
                key: "resonance",
                label: "RESONANCE",
                range: [0.0, 0.95],
                color: TEAL,
                logarithmic: false,
                unit: "",
            },
        );
        self.dials[2].show(
            ui,
            &mut self.color.motion,
            Knob {
                at: [31.0, 277.0],
                key: "motion",
                label: "MOTION",
                range: [0.0, 4.0],
                color: TEAL,
                logarithmic: false,
                unit: "oct",
            },
        );
        self.dials[3].show(
            ui,
            &mut self.color.rate,
            Knob {
                at: [165.0, 277.0],
                key: "rate",
                label: "LFO RATE",
                range: [0.02, 8.0],
                color: TEAL,
                logarithmic: true,
                unit: "Hz",
            },
        );
        ui.pin("out", [284.0, 196.0]);
    }
}

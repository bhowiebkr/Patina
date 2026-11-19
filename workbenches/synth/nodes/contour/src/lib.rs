use panel::{Dial, Knob, MUTED, PINK, inspector, panel, screen};
use patina_sdk::Ui;
use synth_dsp::{Contour, finite};

const SIZE: [f32; 2] = [392.0, 294.0];

/// The amplitude envelope. Attack and release set fade times. Sustain sets the held level.
#[patina_sdk::node(domain = frame, free_layout, full_gui, color = "#71344f")]
pub struct EnvelopeControls {
    contour: Contour,
    dials: [Dial; 4],
}
impl EnvelopeControls {
    pub fn init() -> Self {
        Self {
            contour: Contour::default(),
            dials: Default::default(),
        }
    }
    pub fn process(&mut self, attack: f32, decay: f32, sustain: f32, release: f32) -> Contour {
        self.contour = Contour {
            attack: finite(attack, 0.008, 0.001, 2.0),
            decay: finite(decay, 0.2, 0.005, 2.0),
            sustain: finite(sustain, 0.35, 0.0, 1.0),
            release: finite(release, 0.24, 0.005, 3.0),
        };
        self.contour
    }
    pub fn ui(&mut self, ui: &mut Ui) {
        if inspector(
            ui,
            &[
                "Attack sets the fade into a note.",
                "Decay falls to the sustain level.",
                "Release fades after the gate closes.",
            ],
        ) {
            return;
        }
        {
            let mut c = ui.canvas("Envelope attack decay sustain release", SIZE);
            panel(
                &mut c,
                SIZE,
                "AMPLITUDE ENVELOPE",
                "03   SHAPE EACH NOTE",
                PINK,
            );
            screen(&mut c, [16.0, 72.0, 360.0, 86.0]);
            let e = self.contour;
            let sum = e.attack + e.decay + e.release + 0.25;
            let a = 24.0 + 344.0 * e.attack / sum;
            let d = a + 344.0 * e.decay / sum;
            let s = d + 344.0 * 0.25 / sum;
            let y = 148.0 - e.sustain * 66.0;
            c.path(
                &format!("M 24 148 L {a} 82 L {d} {y} L {s} {y} L 368 148"),
                PINK,
                false,
            );
            for at in [[a, 82.0], [d, y], [s, y]] {
                c.circle(at, 3.0, PINK, true);
            }
            c.text([22.0, 175.0], MUTED, "LEVEL / TIME");
            c.text([270.0, 175.0], MUTED, "GATE RELEASE");
        }
        self.dials[0].show(
            ui,
            &mut self.contour.attack,
            Knob {
                at: [10.0, 179.0],
                key: "attack",
                label: "ATTACK",
                range: [0.001, 2.0],
                color: PINK,
                logarithmic: true,
                unit: "s",
            },
        );
        self.dials[1].show(
            ui,
            &mut self.contour.decay,
            Knob {
                at: [104.0, 179.0],
                key: "decay",
                label: "DECAY",
                range: [0.005, 2.0],
                color: PINK,
                logarithmic: true,
                unit: "s",
            },
        );
        self.dials[2].show(
            ui,
            &mut self.contour.sustain,
            Knob {
                at: [198.0, 179.0],
                key: "sustain",
                label: "SUSTAIN",
                range: [0.0, 1.0],
                color: PINK,
                logarithmic: false,
                unit: "",
            },
        );
        self.dials[3].show(
            ui,
            &mut self.contour.release,
            Knob {
                at: [292.0, 179.0],
                key: "release",
                label: "RELEASE",
                range: [0.005, 3.0],
                color: PINK,
                logarithmic: true,
                unit: "s",
            },
        );
        ui.pin("out", [392.0, 146.0]);
    }
}

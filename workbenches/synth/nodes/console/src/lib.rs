use panel::{AMBER, Dial, INK, Knob, MUTED, TEAL, button, inside, inspector, panel};
use patina_sdk::{Pointer, Ui};

const SIZE: [f32; 2] = [472.0, 246.0];

/// Selects a complete patch and controls the master level. The drum level applies to the phrase patch.
#[patina_sdk::node(domain = frame, free_layout, full_gui, color = "#59366f")]
pub struct Console {
    scene: u8,
    level: f32,
    drums: f32,
    dials: [Dial; 2],
}
impl Console {
    pub fn init() -> Self {
        Self {
            scene: 0,
            level: 0.65,
            drums: 0.65,
            dials: Default::default(),
        }
    }
    pub fn process(&mut self, scene: u8, level: f32, drums: f32) -> (f32, f32, u8) {
        self.scene = scene.min(2);
        self.level = synth_dsp::finite(level, 0.65, 0.0, 1.0);
        self.drums = synth_dsp::finite(drums, 0.65, 0.0, 1.0);
        (self.level, self.drums, self.scene)
    }
    pub fn ui(&mut self, ui: &mut Ui) {
        if inspector(
            ui,
            &[
                "Scene 0: the sequenced synth and drums.",
                "Scene 1: a changing ambient drone.",
                "Scene 2: noise and pitched percussion.",
                "Open the patch subgraphs to change their sound.",
            ],
        ) {
            return;
        }
        let before = self.scene;
        {
            let mut c = ui.canvas(&format!("Workbench scene {}", self.scene), SIZE);
            for p in c.pointer() {
                if let Pointer::Down(at) = *p {
                    for i in 0..3 {
                        if inside(at, [20.0 + i as f32 * 144.0, 72.0, 136.0, 28.0]) {
                            self.scene = i;
                        }
                    }
                }
            }
            panel(
                &mut c,
                SIZE,
                "EMBER / SOUND WORKBENCH",
                "LISTEN. PATCH. SHAPE. SAVE.",
                TEAL,
            );
            for (i, title) in ["PHRASE", "ATMOSPHERE", "IMPACT"].iter().enumerate() {
                button(
                    &mut c,
                    [20.0 + i as f32 * 144.0, 72.0, 136.0, 28.0],
                    title,
                    self.scene == i as u8,
                    TEAL,
                );
            }
            c.text(
                [250.0, 135.0],
                INK,
                match self.scene {
                    1 => "SLOW MOTION / DEEP SPACE",
                    2 => "NOISE / PITCH / TRANSIENTS",
                    _ => "16 STEPS / A SMALL MACHINE",
                },
            );
            c.text([250.0, 161.0], MUTED, "Open a patch to explore it.");
            c.text([250.0, 184.0], MUTED, "Drag modules from Project.");
            c.text([250.0, 207.0], MUTED, "Save keeps your settings.");
        }
        if before != self.scene {
            ui.set_param("scene", self.scene);
        }
        self.dials[0].show(
            ui,
            &mut self.level,
            Knob {
                at: [20.0, 119.0],
                key: "level",
                label: "MASTER",
                range: [0.0, 1.0],
                color: TEAL,
                logarithmic: false,
                unit: "",
            },
        );
        self.dials[1].show(
            ui,
            &mut self.drums,
            Knob {
                at: [124.0, 119.0],
                key: "drums",
                label: "DRUMS",
                range: [0.0, 1.0],
                color: AMBER,
                logarithmic: false,
                unit: "",
            },
        );
        ui.pin("out0", [472.0, 126.0]);
        ui.pin("out1", [472.0, 174.0]);
        ui.pin("out2", [472.0, 222.0]);
    }
}

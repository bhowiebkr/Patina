use super::*;

impl Arrange {
    pub(super) fn sound(&mut self, ui: &mut Ui) -> bool {
        let Some(track) = self.score.tracks.get_mut(self.track) else {
            ui.label("Add a track to design its sound.");
            return false;
        };
        ui.label(&format!("SOUND / {}", track.name));
        let drum = matches!(
            track.instrument,
            Instrument::Kick | Instrument::Snare | Instrument::Hat
        );
        let mut changed = false;
        ui.columns(230.0, |ui| {
            ui.column(|ui| {
                if drum {
                    ui.label("DRUM BODY AND TRANSIENT");
                    changed |= controls(
                        ui,
                        [
                            ("Drum length", &mut track.sound.drum_length, 0.2, 3.0),
                            ("Drum tone", &mut track.sound.drum_tone, 0.0, 1.0),
                            ("Stop release / seconds", &mut track.release, 0.005, 0.3),
                        ],
                    );
                    ui.label("Drums play one shot. Note 42 closes an open hat (46).");
                } else {
                    ui.label("AMPLITUDE ENVELOPE");
                    changed |= controls(
                        ui,
                        [
                            ("Attack / seconds", &mut track.attack, 0.001, 4.0),
                            ("Decay / seconds", &mut track.sound.decay, 0.005, 4.0),
                            ("Sustain", &mut track.sound.sustain, 0.0, 1.0),
                            ("Release / seconds", &mut track.release, 0.005, 8.0),
                        ],
                    );
                }
            });
            if !drum && track.instrument != Instrument::Sampler {
                ui.column(|ui| {
                    ui.label("VOICE FILTER");
                    changed |= controls(
                        ui,
                        [
                            (
                                "Brightness / Hz",
                                &mut track.sound.brightness,
                                40.0,
                                18000.0,
                            ),
                            ("Resonance", &mut track.sound.resonance, 0.0, 0.9),
                            ("Filter sweep / octaves", &mut track.sound.sweep, 0.0, 6.0),
                            (
                                "Sweep decay / seconds",
                                &mut track.sound.sweep_decay,
                                0.005,
                                4.0,
                            ),
                        ],
                    );
                    ui.label("Each note opens its filter. Velocity sets the sweep depth.");
                });
                ui.column(|ui| {
                    ui.label("SOURCE CHARACTER");
                    match track.instrument {
                        Instrument::Rave => {
                            changed |= controls(
                                ui,
                                [
                                    ("Source edge", &mut track.sound.edge, 1.0, 16.0),
                                    ("Fifth level", &mut track.rave.fifth, 0.0, 1.0),
                                    ("Source body / Hz", &mut track.rave.body, 200.0, 8000.0),
                                ],
                            );
                            changed |= number8(ui, "Source bits", &mut track.rave.bits, [4, 16]);
                            ui.label("Pulse melody above a saw 31 semitones lower. Fifth level sets the pulse.");
                        }
                        Instrument::Bass => {
                            changed |= ui
                                .slider("Source edge", &mut track.sound.edge, 1.0..=16.0)
                                .changed();
                            ui.label("A centered sine body supports the shaped saw.");
                        }
                        _ => {
                            changed |= controls(
                                ui,
                                [
                                    ("Unison detune / cents", &mut track.sound.detune, 0.0, 40.0),
                                    ("Stereo width", &mut track.sound.width, 0.0, 1.0),
                                ],
                            );
                            ui.label("Seven band-limited oscillators share one note envelope.");
                        }
                    }
                });
            }
            ui.column(|ui| {
                ui.label("TRACK SPACE AND MIX");
                changed |= controls(
                    ui,
                    [
                        ("Low cut / Hz", &mut track.sound.highpass, 20.0, 2000.0),
                        ("Room", &mut track.sound.room, 0.0, 0.6),
                        ("Beat duck", &mut track.sound.duck, 0.0, 0.9),
                        ("Delay send", &mut track.delay_mix, 0.0, 0.8),
                        ("Delay / beats", &mut track.delay_beats, 0.125, 4.0),
                    ],
                );
                ui.label("Beat duck follows quarter notes. It does not detect kick audio.");
            });
        });
        changed
    }
}

fn controls<const N: usize>(ui: &mut Ui, controls: [(&str, &mut f32, f32, f32); N]) -> bool {
    let mut changed = false;
    for (label, value, lo, hi) in controls {
        changed |= ui.slider(label, value, lo..=hi).changed();
    }
    changed
}

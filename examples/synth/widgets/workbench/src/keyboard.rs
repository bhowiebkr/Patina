use crate::{parameters::SEQUENCE, width};
use panel::{DARK, EDGE, INK, MUTED, TEAL};
use patina_sdk::{Graph, Key, Pointer, Ui};
use std::sync::Arc;
use synth_dsp::{LiveKeys, Sequence};

const CODES: [&str; 17] = [
    "KeyA",
    "KeyW",
    "KeyS",
    "KeyE",
    "KeyD",
    "KeyF",
    "KeyT",
    "KeyG",
    "KeyY",
    "KeyH",
    "KeyU",
    "KeyJ",
    "KeyK",
    "KeyO",
    "KeyL",
    "KeyP",
    "Semicolon",
];
pub struct Keyboard {
    octave: usize,
    velocity: u32,
    held: Vec<u8>,
    pointer: Option<u8>,
    receiver: Option<Arc<LiveKeys>>,
}
impl Default for Keyboard {
    fn default() -> Self {
        Self {
            octave: 3,
            velocity: 100,
            held: Vec::with_capacity(17),
            pointer: None,
            receiver: None,
        }
    }
}
impl Drop for Keyboard {
    fn drop(&mut self) {
        self.release();
    }
}
impl Keyboard {
    fn release(&mut self) {
        self.held.clear();
        self.pointer = None;
        if let Some(keys) = &self.receiver {
            keys.set(None, 127);
        }
    }
    pub fn ui(&mut self, ui: &mut Ui, graph: &mut Graph) {
        let receiver = graph
            .value::<Sequence>(SEQUENCE, "out")
            .map(|s| s.live_keys.clone());
        if let Some(next) = receiver {
            if self
                .receiver
                .as_ref()
                .is_none_or(|old| !Arc::ptr_eq(old, &next))
            {
                self.release();
                self.receiver = Some(next);
            }
        } else {
            self.release();
            self.receiver = None;
        }
        ui.columns(180.0, |ui| {
            ui.column(|ui| {
                if ui
                    .dropdown(
                        "Octave",
                        &mut self.octave,
                        &["C0", "C1", "C2", "C3", "C4", "C5", "C6"],
                    )
                    .changed()
                {
                    self.release();
                }
            });
            ui.column(|ui| {
                ui.slider("Velocity", &mut self.velocity, 1..=127);
            });
            ui.column(|ui| {
                if ui.button("All notes off").clicked() {
                    self.release();
                }
            });
        });
        if self.receiver.is_none() {
            ui.label("Run Ember's main graph. The keyboard connects to its sequencer after the next frame.");
        }
        let w = width(ui);
        let octaves = ((w / 280.0) as usize).clamp(1, 4).min(9 - self.octave);
        let white = w / (7 * octaves) as f32;
        let base = (12 + self.octave * 12) as u8;
        if self
            .receiver
            .as_ref()
            .is_some_and(|keys| keys.note().is_none())
        {
            self.held.clear();
            self.pointer = None;
        }
        let held_before = self.pointer.or_else(|| self.held.last().copied());
        let mut c = ui.canvas(
            "Keyboard A W S E D F T G Y H U J K; click to focus",
            [w, 88.0],
        );
        c.grab();
        for event in c.pointer() {
            match *event {
                Pointer::Down(at) | Pointer::Move(at) => {
                    self.pointer = hit(at, white, octaves).map(|n| base + n);
                }
                Pointer::Up(_) => self.pointer = None,
            }
        }
        for event in c.keys() {
            let (code, down) = match event {
                Key::Down(code) => (code, true),
                Key::Up(code) => (code, false),
            };
            if let Some(index) = CODES.iter().position(|k| *k == code) {
                let note = base + index as u8;
                self.held.retain(|n| *n != note);
                if down {
                    self.held.push(note);
                }
            }
        }
        let held = self.pointer.or_else(|| self.held.last().copied());
        if let Some(keys) = &self.receiver {
            if held != held_before {
                keys.set(held, self.velocity as u8);
            }
            keys.heartbeat();
        }
        for octave in 0..octaves {
            for (i, pitch) in [0, 2, 4, 5, 7, 9, 11].iter().enumerate() {
                let x = (octave * 7 + i) as f32 * white;
                let note = base + octave as u8 * 12 + pitch;
                c.rect(
                    [x, 0.0],
                    [white - 1.0, 86.0],
                    if held == Some(note) { TEAL } else { INK },
                    true,
                );
                if i == 0 {
                    c.text([x + 4.0, 76.0], DARK, &format!("C{}", self.octave + octave));
                }
            }
            for (i, pitch) in [(1, 1), (2, 3), (4, 6), (5, 8), (6, 10)] {
                let x = (octave * 7 + i) as f32 * white - white * 0.3;
                c.rect(
                    [x, 0.0],
                    [white * 0.6, 53.0],
                    if held == Some(base + octave as u8 * 12 + pitch) {
                        TEAL
                    } else {
                        DARK
                    },
                    true,
                );
                c.line([x, 53.0], [x + white * 0.6, 53.0], EDGE);
            }
        }
        c.text([w - 4.0, 0.0], MUTED, "");
        drop(c);
        ui.label("Play A W S E D F T G Y H U J K O L P ;. One voice. MIDI is not connected.");
    }
}

fn hit([x, y]: [f32; 2], white: f32, octaves: usize) -> Option<u8> {
    if x < 0.0 || x >= white * (octaves * 7) as f32 || !(0.0..88.0).contains(&y) {
        return None;
    }
    let octave = (x / (white * 7.0)) as usize;
    let local = x - octave as f32 * white * 7.0;
    if y < 53.0 {
        for (i, pitch) in [(1, 1), (2, 3), (4, 6), (5, 8), (6, 10)] {
            if (local - i as f32 * white).abs() < white * 0.3 {
                return Some(octave as u8 * 12 + pitch);
            }
        }
    }
    Some(octave as u8 * 12 + [0, 2, 4, 5, 7, 9, 11][(local / white) as usize])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn black_keys_win_and_outside_releases() {
        assert_eq!(hit([40.0, 20.0], 40.0, 2), Some(1));
        assert_eq!(hit([40.0, 70.0], 40.0, 2), Some(2));
        assert_eq!(hit([280.0, 70.0], 40.0, 2), Some(12));
        assert_eq!(hit([560.0, 20.0], 40.0, 2), None);
    }
}

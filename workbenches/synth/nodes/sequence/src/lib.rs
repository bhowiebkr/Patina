use panel::{
    AMBER, DARK, Dial, EDGE, INK, Knob, MUTED, PINK, TEAL, button, inside, inspector, panel,
    rounded,
};
use patina_sdk::{Key, Pointer, Ui};
use synth_dsp::{NAMES, PATTERNS, Sequence, finite, parse_pattern, pattern_text};

const SIZE: [f32; 2] = [472.0, 500.0];
const KEYS: [&str; 8] = [
    "KeyA", "KeyS", "KeyD", "KeyF", "KeyG", "KeyH", "KeyJ", "KeyK",
];

/// Ember's score. Click a pitch cell or drum pad. Play the keys A S D F G H J K on its focused keyboard.
#[patina_sdk::node(domain = frame, free_layout, full_gui, color = "#655219")]
pub struct Sequencer {
    settings: Sequence,
    held: [bool; 8],
    dials: [Dial; 3],
}
impl Sequencer {
    pub fn init() -> Self {
        Self {
            settings: Sequence::default(),
            held: [false; 8],
            dials: Default::default(),
        }
    }

    #[allow(clippy::too_many_arguments)] // Each argument is an editable graph input.
    pub fn process(
        &mut self,
        bpm: f32,
        swing: f32,
        gate: f32,
        playing: bool,
        pattern: &str,
        kick: u16,
        snare: u16,
        hat: u16,
    ) -> Sequence {
        self.settings.bpm = finite(bpm, 112.0, 40.0, 220.0);
        self.settings.swing = finite(swing, 0.12, 0.0, 0.45);
        self.settings.gate = finite(gate, 0.58, 0.05, 0.95);
        self.settings.playing = playing;
        self.settings.notes = parse_pattern(pattern);
        self.settings.drums = [kick, snare, hat];
        self.settings.clone()
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        if inspector(
            ui,
            &[
                "Click a pitch cell to add or remove a note.",
                "BD: kick. SD: snare. HH: hi-hat.",
                "Gate sets note length within each step.",
                "Hold stops the score. The keyboard stays live.",
            ],
        ) {
            return;
        }
        let (old_notes, old_drums, old_play) = (
            self.settings.notes,
            self.settings.drums,
            self.settings.playing,
        );
        {
            let mut c = ui.canvas(
                &format!(
                    "Ember sequencer {}",
                    if self.settings.playing {
                        "playing"
                    } else {
                        "hold"
                    }
                ),
                SIZE,
            );
            c.grab();
            for event in c.pointer() {
                if let Pointer::Down(at) = *event {
                    if inside(at, [340.0, 16.0, 112.0, 28.0]) {
                        self.settings.playing = !self.settings.playing;
                    }
                    for (i, pattern) in PATTERNS.iter().enumerate() {
                        if inside(at, [20.0 + i as f32 * 144.0, 72.0, 136.0, 27.0]) {
                            self.settings.notes = parse_pattern(pattern);
                        }
                    }
                    if inside(at, [44.0, 119.0, 408.0, 128.0]) {
                        let step = ((at[0] - 44.0) / 25.5) as usize;
                        let note = 7 - ((at[1] - 119.0) / 16.0) as i8;
                        self.settings.notes[step] = if self.settings.notes[step] == note {
                            -1
                        } else {
                            note
                        };
                    }
                    if inside(at, [44.0, 271.0, 408.0, 60.0]) {
                        let step = ((at[0] - 44.0) / 25.5) as usize;
                        let lane = ((at[1] - 271.0) / 20.0) as usize;
                        self.settings.drums[lane] ^= 1 << step;
                    }
                }
            }
            draw(&mut c, &self.settings);
        }
        if old_notes != self.settings.notes {
            ui.set_param("pattern", pattern_text(&self.settings.notes));
        }
        for (i, key) in ["kick", "snare", "hat"].iter().enumerate() {
            if old_drums[i] != self.settings.drums[i] {
                ui.set_param(key, self.settings.drums[i]);
            }
        }
        if old_play != self.settings.playing {
            ui.set_param("playing", self.settings.playing);
        }
        self.dials[0].show(
            ui,
            &mut self.settings.bpm,
            Knob {
                at: [20.0, 344.0],
                key: "bpm",
                label: "TEMPO",
                range: [40.0, 220.0],
                color: AMBER,
                logarithmic: false,
                unit: "BPM",
            },
        );
        self.dials[1].show(
            ui,
            &mut self.settings.swing,
            Knob {
                at: [116.0, 344.0],
                key: "swing",
                label: "SWING",
                range: [0.0, 0.45],
                color: AMBER,
                logarithmic: false,
                unit: "",
            },
        );
        self.dials[2].show(
            ui,
            &mut self.settings.gate,
            Knob {
                at: [212.0, 344.0],
                key: "gate",
                label: "GATE",
                range: [0.05, 0.95],
                color: AMBER,
                logarithmic: false,
                unit: "",
            },
        );
        self.keyboard(ui);
        ui.pin("out", [472.0, 252.0]);
    }

    fn keyboard(&mut self, ui: &mut Ui) {
        ui.place([314.0, 354.0]);
        let mut c = ui.canvas("Keyboard A S D F G H J K", [138.0, 126.0]);
        c.grab();
        for event in c.pointer() {
            match *event {
                Pointer::Down([x, y]) if (22.0..88.0).contains(&y) && (0.0..138.0).contains(&x) => {
                    self.held.fill(false);
                    self.held[(x / 17.25) as usize] = true;
                    self.settings.strike += 1;
                }
                Pointer::Up(_) => self.held.fill(false),
                _ => {}
            }
        }
        for event in c.keys() {
            let (key, down) = match event {
                Key::Down(k) => (k, true),
                Key::Up(k) => (k, false),
            };
            if let Some(i) = KEYS.iter().position(|k| *k == key) {
                if down && !self.held[i] {
                    self.settings.strike += 1;
                }
                self.held[i] = down;
            }
        }
        self.settings.keyboard = self.held.iter().rposition(|&v| v).map_or(-1, |i| i as i8);
        c.text([0.0, 12.0], MUTED, "PLAY / C MINOR");
        for (i, key) in ["A", "S", "D", "F", "G", "H", "J", "K"].iter().enumerate() {
            let x = i as f32 * 17.25;
            rounded(
                &mut c,
                [x, 22.0, 15.0, 66.0],
                2.0,
                if self.held[i] { AMBER } else { [208, 208, 201] },
            );
            c.text([x + 3.0, 78.0], DARK, key);
        }
        c.text([0.0, 110.0], MUTED, "Click, then play");
    }
}

fn draw(c: &mut patina_sdk::Canvas, s: &Sequence) {
    panel(c, SIZE, "EMBER / SEQUENCER", "01   COMPOSE A PHRASE", AMBER);
    button(
        c,
        [340.0, 16.0, 112.0, 28.0],
        if s.playing { "PLAYING" } else { "HOLD" },
        s.playing,
        AMBER,
    );
    for (i, name) in ["NIGHT DRIVE", "GLASS STEPS", "ACID LINE"]
        .iter()
        .enumerate()
    {
        button(
            c,
            [20.0 + i as f32 * 144.0, 72.0, 136.0, 27.0],
            name,
            s.notes == parse_pattern(PATTERNS[i]),
            AMBER,
        );
    }
    for (note, name) in NAMES.iter().enumerate() {
        let y = 119.0 + (7 - note) as f32 * 16.0;
        c.text([10.0, y + 12.0], MUTED, name);
        for step in 0..16 {
            let x = 44.0 + step as f32 * 25.5;
            let active = s.notes[step] == note as i8;
            rounded(
                c,
                [x, y, 23.5, 14.0],
                2.0,
                if active {
                    AMBER
                } else if step == s.position.step() && s.playing {
                    [69, 59, 43]
                } else if step % 4 == 0 {
                    [48, 48, 48]
                } else {
                    DARK
                },
            );
            if active {
                c.line([x + 4.0, y + 7.0], [x + 19.0, y + 7.0], DARK);
            }
        }
    }
    for i in 0..16 {
        c.text(
            [47.0 + i as f32 * 25.5, 261.0],
            if i == s.position.step() { AMBER } else { MUTED },
            &format!("{:02}", i + 1),
        );
    }
    for (row, (label, color)) in [("BD", AMBER), ("SD", PINK), ("HH", TEAL)]
        .iter()
        .enumerate()
    {
        let y = 271.0 + row as f32 * 20.0;
        c.text([13.0, y + 13.0], MUTED, label);
        for step in 0..16 {
            let on = s.drums[row] & (1 << step) != 0;
            rounded(
                c,
                [44.0 + step as f32 * 25.5, y, 23.5, 16.0],
                3.0,
                if on { *color } else { EDGE },
            );
            if on {
                c.circle([55.5 + step as f32 * 25.5, y + 8.0], 2.0, DARK, true);
            }
        }
    }
    c.line([20.0, 337.0], [452.0, 337.0], EDGE);
    c.text(
        [20.0, 480.0],
        INK,
        "16 STEPS   /   3 DRUM LANES   /   48 kHz",
    );
}

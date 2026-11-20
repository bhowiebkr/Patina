use panel::{INK, MUTED, VIOLET, panel};
use patina_sdk::{Mode, Ui};
use std::time::{Duration, Instant};
use synth_dsp::{
    RATE,
    capture::Stream,
    spectrogram::{COLUMNS, History, WINDOW},
};

mod display;

// A node is 480 px wide at most (spec 4.3). The image fills the width between equal margins.
const SIZE: [f32; 2] = [472.0, 452.0];
const PLOT: [f32; 4] = [56.0, 146.0, SIZE[0] - 112.0, (SIZE[0] - 112.0) / 2.0];
const STATUS: f32 = PLOT[1] + PLOT[3] + 94.0;

/// A continuous frequency history. Connect Audio Capture to its source input.
#[patina_sdk::node(domain = frame, free_layout, full_gui, color = "#59366f")]
pub struct Spectrogram {
    history: History,
    frozen: bool,
    range: usize,
    span: usize,
    floor: usize,
    pixels: Vec<u8>,
    updated: Instant,
}

impl Spectrogram {
    pub fn init() -> Self {
        Self {
            history: History::default(),
            frozen: false,
            range: 1,
            span: 1,
            floor: 1,
            pixels: vec![0; 256 * 128 * 4],
            updated: Instant::now(),
        }
    }

    pub fn process(&mut self, source: &Stream) -> f32 {
        if self.history.update(source, self.frozen) {
            self.updated = Instant::now();
        }
        self.history.peak
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        // The Inspector is narrower than the node, so it stacks the controls without places.
        let flow = ui.mode() == Mode::Large;
        let place = |ui: &mut Ui, at: [f32; 2]| {
            if !flow {
                ui.place(at);
            }
        };
        if flow {
            ui.label(self.status());
        } else {
            let mut c = ui.canvas("Spectrogram frequency and time axes", SIZE);
            panel(
                &mut c,
                SIZE,
                "SIGNAL / SPECTROGRAM",
                "FREQUENCY OVER TIME / BRIGHTER MEANS LOUDER",
                VIOLET,
            );
            display::axes(&mut c, PLOT, self.max_hz(), self.columns(), self.floor_db());
            c.text([16.0, STATUS], INK, self.status());
            c.text(
                [16.0, STATUS + 18.0],
                MUTED,
                format!(
                    "2048 FFT / 23.4 Hz / {:.1} dBFS peak",
                    synth_dsp::spectrum::db(self.history.peak)
                ),
            );
        }

        place(ui, [16.0, 70.0]);
        ui.tabs(
            "Frequency range",
            &mut self.range,
            &["2 kHz", "5 kHz", "12 kHz", "24 kHz"],
        );
        place(ui, [300.0, 70.0]);
        if ui.toggle("Freeze", &mut self.frozen).changed() && !self.frozen {
            self.history.clear();
        }
        place(ui, [396.0, 70.0]);
        if ui.button("Clear").clicked() {
            self.history.clear();
        }
        place(ui, [16.0, 104.0]);
        ui.tabs(
            "Time span",
            &mut self.span,
            &["5.5 seconds", "10.9 seconds"],
        );
        place(ui, [230.0, 104.0]);
        ui.tabs(
            "Level floor",
            &mut self.floor,
            &["-48 dB", "-72 dB", "-96 dB"],
        );

        let (hz, columns, floor) = (self.max_hz(), self.columns(), self.floor_db());
        display::pixels(&self.history, &mut self.pixels, hz, columns, floor);
        place(ui, [PLOT[0], PLOT[1]]);
        ui.image(
            "Spectrogram: time right, frequency up",
            [256, 128],
            &self.pixels,
        );
        if !flow {
            ui.pin("source", [0.0, 146.0]);
            ui.pin("out", [SIZE[0], STATUS]);
        }
    }

    fn max_hz(&self) -> f32 {
        [2000.0, 5000.0, 12000.0, 24000.0][self.range.min(3)]
    }
    fn columns(&self) -> usize {
        [256, COLUMNS][self.span.min(1)]
    }
    fn floor_db(&self) -> f32 {
        [-48.0, -72.0, -96.0][self.floor.min(2)]
    }
    fn status(&self) -> &'static str {
        if self.frozen {
            "FROZEN / resume starts a new history"
        } else if self.history.columns == 0 && self.history.written > 0 {
            "WAITING / new audio window"
        } else if self.history.columns == 0 {
            "Connect Audio Capture and run"
        } else if self.updated.elapsed() > Duration::from_millis(350) {
            "NO NEW AUDIO / history retained"
        } else if self.history.resets > 0 {
            "LIVE / history reset after missed capture"
        } else {
            "LIVE / continuous audio capture"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use synth_dsp::capture::Capture;

    #[test]
    fn image_controls_freeze_and_clear_real_history() {
        let mut capture = Capture::new("node test");
        let mut node = Spectrogram::init();
        capture.push(&[0.25; 8192]);
        node.process(&capture.stream());
        let mut ui = Ui::new(0, "");
        node.ui(&mut ui);
        assert!(ui.tree().contains("image\tSpectrogram:"));
        let green = |x: usize, y: usize| node.pixels[(y * 256 + x) * 4 + 1];
        assert!(green(255, 127) > 150, "DC must appear at the lower right");
        assert!(
            green(255, 0) < 50 && green(0, 127) < 50,
            "quiet and empty regions stay dark"
        );
        let freeze = ui
            .tree()
            .lines()
            .position(|l| l.starts_with("toggle\tFreeze"))
            .unwrap();
        let clear = ui
            .tree()
            .lines()
            .position(|l| l.starts_with("button\tClear"))
            .unwrap();
        node.ui(&mut Ui::new(0, &format!("{freeze} 1")));
        capture.push(&[0.0; 8192]);
        node.process(&capture.stream());
        assert_eq!(node.history.written, 8192);
        node.ui(&mut Ui::new(0, &format!("{clear} 1")));
        assert_eq!(node.history.columns, 0);
        node.ui(&mut Ui::new(0, &format!("{freeze} 0")));
        node.process(&capture.stream());
        assert_eq!(node.history.written, 16384);
    }
}

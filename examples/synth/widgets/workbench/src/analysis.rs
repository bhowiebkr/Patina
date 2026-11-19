use crate::{
    spectrum::{Spectrum, db},
    width,
};
use panel::{AMBER, DARK, EDGE, MUTED, TEAL, VIOLET};
use patina_sdk::{Canvas, Graph, Ui};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use synth_dsp::capture::{CAPACITY, Signal, signals};

pub struct Analysis {
    source: usize,
    compare: usize,
    mode: usize,
    window: usize,
    freeze: bool,
    trigger: bool,
    settings: bool,
    primary: Reading,
    secondary: Reading,
}
impl Default for Analysis {
    fn default() -> Self {
        Self {
            source: 0,
            compare: 0,
            mode: 0,
            window: 2,
            freeze: false,
            trigger: true,
            settings: false,
            primary: Reading::default(),
            secondary: Reading::default(),
        }
    }
}

pub(crate) struct Reading {
    signal: Option<Arc<Signal>>,
    pub samples: Vec<f32>,
    pending: Vec<f32>,
    spectrum: Spectrum,
    count: u64,
    at: Instant,
    pub peak: f32,
    pub rms: f32,
    hold: f32,
    clips: u64,
    length: usize,
}
impl Default for Reading {
    fn default() -> Self {
        Self {
            signal: None,
            samples: vec![0.0; CAPACITY],
            pending: vec![0.0; CAPACITY],
            spectrum: Spectrum::default(),
            count: 0,
            at: Instant::now(),
            peak: 0.0,
            rms: 0.0,
            hold: 0.0,
            clips: 0,
            length: 8192,
        }
    }
}
impl Reading {
    pub fn update(&mut self, signal: Option<Arc<Signal>>, length: usize, frozen: bool) {
        let changed = match (&self.signal, &signal) {
            (Some(a), Some(b)) => !Arc::ptr_eq(a, b),
            (None, None) => false,
            _ => true,
        };
        if changed {
            self.count = 0;
            self.samples.fill(0.0);
            self.peak = 0.0;
            self.rms = 0.0;
            self.hold = 0.0;
            self.clips = 0;
        }
        self.signal = signal;
        if frozen {
            return;
        }
        let Some(signal) = &self.signal else {
            return;
        };
        if let Some(count) = signal.read(&mut self.pending[..length])
            && (count != self.count || length != self.length)
        {
            std::mem::swap(&mut self.samples, &mut self.pending);
            self.count = count;
            self.length = length;
            self.at = Instant::now();
            self.peak = self.samples[..length]
                .iter()
                .fold(0.0_f32, |v, s| v.max(s.abs()));
            self.rms =
                (self.samples[..length].iter().map(|s| s * s).sum::<f32>() / length as f32).sqrt();
            self.hold = self.hold.max(self.peak);
            self.clips += u64::from(self.peak >= 1.0);
            self.spectrum.update(&self.samples[..length]);
        }
    }
    pub fn status(&self) -> &'static str {
        if self.signal.is_none() {
            "No source"
        } else if self.count == 0 {
            "Waiting for samples"
        } else if self.at.elapsed() > Duration::from_millis(350) {
            "Stale / engine paused or stopped"
        } else {
            "Live"
        }
    }
    pub fn meters(&self, frozen: bool) -> String {
        format!(
            "{} | Peak {:.1} dBFS | RMS {:.1} dBFS | Hold {:.1} dBFS{}",
            if frozen { "FROZEN" } else { self.status() },
            db(self.peak),
            db(self.rms),
            db(self.hold),
            if self.clips > 0 { " | CLIPPED" } else { "" }
        )
    }
}

impl Analysis {
    pub fn ui(&mut self, ui: &mut Ui, _: &mut Graph) {
        let sources = signals();
        if self.primary.signal.is_none() {
            self.source = sources.iter().position(|s| s.name == "Output").unwrap_or(0);
        }
        if sources.is_empty() {
            ui.label("Run the graph to capture audio from its oscillator, envelope, filter, echo, drums, and output.");
            return;
        }
        self.source = self.source.min(sources.len() - 1);
        self.compare = self.compare.min(sources.len());
        let names: Vec<_> = sources
            .iter()
            .enumerate()
            .map(|(i, s)| format!("{} / {}", s.name, i + 1))
            .collect();
        let mut comparisons = vec!["None"];
        comparisons.extend(names.iter().map(String::as_str));
        let wide = width(ui) > 900.0;
        if !wide {
            self.mode = self.mode.min(1);
        }
        ui.columns(150.0, |ui| {
            ui.column(|ui| {
                ui.dropdown(
                    "Source",
                    &mut self.source,
                    &names.iter().map(String::as_str).collect::<Vec<_>>(),
                );
            });
            ui.column(|ui| {
                ui.dropdown("Compare", &mut self.compare, &comparisons);
            });
            ui.column(|ui| {
                ui.dropdown(
                    "Display",
                    &mut self.mode,
                    if wide {
                        &["Waveform", "Spectrum", "Both"]
                    } else {
                        &["Waveform", "Spectrum"]
                    },
                );
            });
            ui.column(|ui| {
                ui.toggle("Freeze", &mut self.freeze);
            });
        });
        let length = [1024, 4096, 8192][self.window];
        self.primary
            .update(Some(sources[self.source].clone()), length, self.freeze);
        self.secondary.update(
            self.compare
                .checked_sub(1)
                .and_then(|i| sources.get(i))
                .cloned(),
            length,
            self.freeze,
        );
        let w = width(ui);
        let h = (ui.available_size().unwrap_or([900.0, 220.0])[1] - 116.0).clamp(54.0, 340.0);
        let mut c = ui.canvas("Audio analysis; teal source and violet comparison", [w, h]);
        if self.mode == 2 && w > 900.0 {
            self.plot(&mut c, [0.0, 0.0, w * 0.5 - 6.0, h], false);
            self.plot(&mut c, [w * 0.5 + 6.0, 0.0, w * 0.5 - 6.0, h], true);
        } else {
            self.plot(&mut c, [0.0, 0.0, w, h], self.mode == 1);
        }
        drop(c);
        ui.label(&self.primary.meters(self.freeze));
        ui.section("Analysis settings", &mut self.settings, |ui| {
            ui.columns(180.0, |ui| {
                ui.column(|ui| {
                    ui.dropdown(
                        "Window",
                        &mut self.window,
                        &["1024 / 21 ms", "4096 / 85 ms", "8192 / 171 ms"],
                    );
                });
                ui.column(|ui| {
                    ui.toggle("Trigger on rising zero", &mut self.trigger);
                });
                ui.column(|ui| {
                    if ui.button("Reset peak hold").clicked() {
                        self.primary.hold = 0.0;
                        self.primary.clips = 0;
                    }
                });
            })
        });
    }
    fn plot(&self, c: &mut Canvas, [x, y, w, h]: [f32; 4], spectrum: bool) {
        c.rect([x, y], [w, h], DARK, true);
        let area = [x + 35.0, y + 10.0, w - 44.0, h - 32.0];
        for i in 0..5 {
            let gy = area[1] + area[3] * i as f32 / 4.0;
            c.line([area[0], gy], [x + w - 8.0, gy], EDGE);
        }
        c.text(
            [x + 2.0, y + 12.0],
            MUTED,
            if spectrum { "0 dB" } else { "+1" },
        );
        c.text(
            [x + 2.0, y + h - 20.0],
            MUTED,
            if spectrum { "-96" } else { "-1" },
        );
        if spectrum {
            for (hz, text) in [
                (20.0_f32, "20 Hz"),
                (100.0, "100"),
                (1000.0, "1k"),
                (24000.0, "24k Hz"),
            ] {
                c.text(
                    [
                        area[0] + area[2] * (hz / 20.0).ln() / 1200.0_f32.ln()
                            - if hz == 24000.0 { 42.0 } else { 0.0 },
                        y + h - 3.0,
                    ],
                    MUTED,
                    text,
                );
            }
        } else {
            c.text([area[0], y + h - 3.0], MUTED, "0 ms");
            c.text(
                [x + w - 65.0, y + h - 3.0],
                MUTED,
                &format!(
                    "{:.1} ms",
                    self.primary.length as f32 / if self.trigger { 96.0 } else { 48.0 }
                ),
            );
        }
        let start = if self.trigger {
            self.primary.samples[..self.primary.length]
                .windows(2)
                .take(self.primary.length / 2)
                .position(|p| p[0] <= 0.0 && p[1] > 0.0)
                .unwrap_or(0)
        } else {
            0
        };
        for (r, color) in [(&self.primary, TEAL), (&self.secondary, VIOLET)] {
            if r.signal.is_none() || r.count == 0 {
                continue;
            }
            if spectrum {
                trace(c, area, &r.spectrum.bands, -96.0, 0.0, color);
            } else {
                let samples = &r.samples[..r.length];
                let count = if self.trigger { r.length / 2 } else { r.length };
                trace(
                    c,
                    area,
                    &samples[start.min(r.length - count)..start.min(r.length - count) + count],
                    -1.0,
                    1.0,
                    color,
                );
            }
        }
    }
}

fn trace(
    c: &mut Canvas,
    [x, y, w, h]: [f32; 4],
    samples: &[f32],
    low: f32,
    high: f32,
    color: [u8; 3],
) {
    let count = samples.len().min(192);
    let mut previous = None;
    for i in 0..count {
        let at = i * (samples.len() - 1) / (count - 1);
        let p = [
            x + i as f32 / (count - 1) as f32 * w,
            y + h * (1.0 - ((samples[at] - low) / (high - low)).clamp(0.0, 1.0)),
        ];
        if let Some(last) = previous {
            c.line(last, p, color);
        }
        previous = Some(p);
    }
}

#[derive(Default)]
pub struct Audio {
    meter: Reading,
}
impl Audio {
    pub fn ui(&mut self, ui: &mut Ui, graph: &mut Graph) {
        use crate::parameters::*;
        crate::title(ui, "EMBER / AUDIO", "Mono output / 48 kHz / 256 samples");
        self.meter.update(
            signals().into_iter().find(|s| s.name == "Output"),
            4096,
            false,
        );
        let w = width(ui);
        let mut c = ui.canvas("Output peak meter in dBFS", [w, 50.0]);
        c.rect([0.0, 6.0], [w, 16.0], DARK, true);
        let level = ((db(self.meter.peak) + 60.0) / 60.0).clamp(0.0, 1.0);
        if self.meter.status() == "Live" {
            c.rect(
                [0.0, 6.0],
                [w * level, 16.0],
                if self.meter.peak >= 1.0 { AMBER } else { TEAL },
                true,
            );
        }
        c.text([0.0, 40.0], MUTED, "-60 dBFS");
        c.text([w - 44.0, 40.0], MUTED, "0 dBFS");
        drop(c);
        ui.label(&self.meter.meters(false));
        crate::parameter(ui, graph, MASTER);
        if available(graph, OUTPUT, "nodes/output", "audible") {
            let mut on = graph.param(OUTPUT, "audible") == Some("true");
            if ui.toggle("Device output enabled", &mut on).changed() {
                graph.set_param(OUTPUT, "audible", on);
            }
        }
        ui.label("The meter measures the signal before device mute.");
        ui.label("Device: system default. Connection notices appear in Output. Device availability is not exposed to this view.");
        if ui.button("Reset clipping and peak hold").clicked() {
            self.meter.hold = 0.0;
            self.meter.clips = 0;
        }
    }
}

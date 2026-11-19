//! Continuous STFT history. Call this on the frame thread, never the audio thread.
use crate::{
    RATE,
    capture::{CAPACITY, Stream},
    spectrum::Spectrum,
};

pub const WINDOW: usize = 2048;
pub const HOP: usize = 1024;
pub const COLUMNS: usize = 512;
const BINS: usize = WINDOW / 2 + 1;

pub struct History {
    source: Stream,
    pending: Box<[f32]>,
    ring: [f32; WINDOW],
    ordered: [f32; WINDOW],
    spectrum: Spectrum,
    levels: Box<[u8]>,
    sample: usize,
    write: usize,
    pub columns: usize,
    pub written: u64,
    pub resets: u32,
    pub peak: f32,
}

impl Default for History {
    fn default() -> Self {
        Self {
            source: Stream::default(),
            pending: vec![0.0; CAPACITY].into_boxed_slice(),
            ring: [0.0; WINDOW],
            ordered: [0.0; WINDOW],
            spectrum: Spectrum::default(),
            levels: vec![0; COLUMNS * BINS].into_boxed_slice(),
            sample: 0,
            write: 0,
            columns: 0,
            written: 0,
            resets: 0,
            peak: 0.0,
        }
    }
}

impl History {
    pub fn clear(&mut self) {
        self.columns = 0;
        self.write = 0;
        self.sample = 0;
        self.peak = 0.0;
    }

    /// Returns true only when new samples arrive. A missed capture resets the time axis.
    pub fn update(&mut self, source: &Stream, frozen: bool) -> bool {
        if !self.source.same_source(source) {
            self.source = source.clone();
            self.clear();
            self.written = 0;
            self.resets = 0;
        }
        if frozen {
            return false;
        }
        let Some((written, available)) = source.read_available(&mut self.pending) else {
            return false;
        };
        if written == self.written {
            return false;
        }
        let delta = written.saturating_sub(self.written);
        if self.written != 0 && (delta > CAPACITY as u64 || written < self.written) {
            self.clear();
            self.resets = self.resets.saturating_add(1);
        }
        let count = delta.min(available as u64) as usize;
        self.peak = 0.0;
        for i in available - count..available {
            let value = self.pending[i];
            let value = if value.is_finite() { value } else { 0.0 };
            self.peak = self.peak.max(value.abs());
            self.ring[self.sample % WINDOW] = value;
            self.sample += 1;
            if self.sample >= WINDOW && self.sample.is_multiple_of(HOP) {
                let start = self.sample % WINDOW;
                for j in 0..WINDOW {
                    self.ordered[j] = self.ring[(start + j) % WINDOW];
                }
                self.spectrum.analyze(&self.ordered);
                for bin in 0..BINS {
                    let db = self.spectrum.bin_db(bin, WINDOW);
                    self.levels[self.write * BINS + bin] =
                        ((db + 100.0) * 2.0).round().clamp(0.0, 255.0) as u8;
                }
                self.write = (self.write + 1) % COLUMNS;
                self.columns = (self.columns + 1).min(COLUMNS);
            }
        }
        self.written = written;
        true
    }

    /// Column zero is the oldest visible column. Empty columns precede the captured history.
    pub fn level(&self, column: usize, bin: usize, span: usize) -> f32 {
        let span = span.clamp(1, COLUMNS);
        let visible = self.columns.min(span);
        if column >= span || column < span - visible || bin >= BINS {
            return -100.0;
        }
        let index = (self.write + COLUMNS - span + column) % COLUMNS;
        f32::from(self.levels[index * BINS + bin]) * 0.5 - 100.0
    }

    pub fn seconds(span: usize) -> f32 {
        span as f32 * HOP as f32 / RATE
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::Capture;

    #[test]
    fn continuous_history_keeps_pitch_level_and_time_without_allocations() {
        let mut capture = Capture::new("spectrogram test");
        let source = capture.stream();
        let mut history = History::default();
        let mut times = Vec::with_capacity(800);
        for block in 0..2400 {
            let values = std::array::from_fn::<_, 256, _>(|i| {
                0.5 * (std::f32::consts::TAU * (block * 256 + i) as f32 * 10.0 / WINDOW as f32)
                    .sin()
            });
            crate::tests::allocations::COUNT.set(0);
            crate::tests::allocations::ENABLED.set(true);
            capture.push(&values);
            let start = std::time::Instant::now();
            if block % 3 == 2 {
                history.update(&source, false);
            }
            let elapsed = start.elapsed().as_secs_f64() * 1000.0;
            crate::tests::allocations::ENABLED.set(false);
            assert_eq!(crate::tests::allocations::COUNT.get(), 0);
            if block % 3 == 2 {
                times.push(elapsed);
            }
        }
        assert_eq!(history.columns, COLUMNS);
        assert_eq!(history.written, 614400);
        assert_eq!(history.resets, 0);
        assert!((history.level(COLUMNS - 1, 10, COLUMNS) + 6.0).abs() < 0.6);
        assert!(history.level(COLUMNS - 1, 30, COLUMNS) < -80.0);
        times.sort_by(f64::total_cmp);
        println!(
            "Spectrogram frame update: mean {:.3} ms, p95 {:.3} ms; zero allocations",
            times.iter().sum::<f64>() / times.len() as f64,
            times[times.len() * 95 / 100]
        );
    }

    #[test]
    fn freeze_source_changes_and_overruns_do_not_invent_history() {
        let mut a = Capture::new("a");
        let b = Capture::new("b");
        let mut h = History::default();
        a.push(&[0.0; CAPACITY]);
        assert!(h.update(&a.stream(), false));
        assert_eq!(h.columns, 7);
        a.push(&[0.0; 256]);
        assert!(!h.update(&a.stream(), true));
        assert_eq!(h.written, CAPACITY as u64);
        a.push(&[f32::NAN; CAPACITY]);
        h.update(&a.stream(), false);
        assert_eq!(h.resets, 1);
        assert_eq!(h.columns, 7);
        assert_eq!(h.peak, 0.0);
        h.update(&b.stream(), false);
        assert_eq!(h.columns, 0);
        assert_eq!(h.written, 0);
        drop(b);
        assert!(!h.update(&Stream::default(), false));
    }

    #[test]
    fn the_first_window_starts_before_the_capture_buffer_fills() {
        let mut capture = Capture::new("start");
        let mut history = History::default();
        for _ in 0..WINDOW / 256 {
            capture.push(&[0.5; 256]);
            assert!(history.update(&capture.stream(), false));
        }
        assert_eq!(history.written, WINDOW as u64);
        assert_eq!(history.columns, 1);
        assert_eq!(history.level(COLUMNS - 1, 0, COLUMNS), -6.0);
        history.clear();
        assert!(!history.update(&capture.stream(), false));
        assert_eq!(history.columns, 0, "clear must not recapture old samples");
    }
}

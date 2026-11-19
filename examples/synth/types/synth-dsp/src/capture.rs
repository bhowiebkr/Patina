//! Continuous analysis capture. Only the owning audio node writes a signal.
use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering::SeqCst},
};

pub const CAPACITY: usize = 8192;

pub struct Signal {
    pub name: String,
    samples: Box<[AtomicU32]>,
    sequence: AtomicU64,
    written: AtomicU64,
    alive: AtomicBool,
}

impl Signal {
    /// Copies a coherent recent window. None means no samples or a concurrent overwrite.
    pub fn read(&self, out: &mut [f32]) -> Option<u64> {
        if out.len() > CAPACITY || !self.alive.load(SeqCst) {
            return None;
        }
        let length = out.len();
        for _ in 0..2 {
            let before = self.sequence.load(SeqCst);
            let written = self.written.load(SeqCst);
            if before & 1 != 0 || written < out.len() as u64 {
                return None;
            }
            for (i, value) in out.iter_mut().enumerate() {
                *value = f32::from_bits(
                    self.samples[(written as usize + CAPACITY - length + i) % CAPACITY]
                        .load(SeqCst),
                );
            }
            if before == self.sequence.load(SeqCst) {
                return Some(written);
            }
        }
        None
    }
}

#[derive(Default)]
struct Registry(Mutex<Vec<Weak<Signal>>>);

pub fn signals() -> Vec<Arc<Signal>> {
    let mut entries = patina_sdk::shared(Registry::default)
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    entries.retain(|entry| {
        entry
            .upgrade()
            .is_some_and(|signal| signal.alive.load(SeqCst))
    });
    entries.iter().filter_map(Weak::upgrade).collect()
}

pub struct Capture {
    signal: Arc<Signal>,
    written: u64,
}

impl Capture {
    /// Allocate and register during node initialization, before the audio thread starts.
    pub fn new(name: &str) -> Self {
        let signal = Arc::new(Signal {
            name: name.into(),
            samples: (0..CAPACITY).map(|_| AtomicU32::new(0)).collect(),
            sequence: AtomicU64::new(0),
            written: AtomicU64::new(0),
            alive: AtomicBool::new(true),
        });
        patina_sdk::shared(Registry::default)
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(Arc::downgrade(&signal));
        Self { signal, written: 0 }
    }

    /// No locks, allocations, or retries on the audio thread.
    pub fn push(&mut self, samples: &[f32]) {
        self.signal.sequence.fetch_add(1, SeqCst);
        for &sample in samples {
            self.signal.samples[self.written as usize % CAPACITY].store(sample.to_bits(), SeqCst);
            self.written = self.written.wrapping_add(1);
        }
        self.signal.written.store(self.written, SeqCst);
        self.signal.sequence.fetch_add(1, SeqCst);
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.signal.alive.store(false, SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capture_wraps_without_gaps_and_never_allocates_when_writing() {
        let mut capture = Capture::new("test");
        let mut out = [0.0; 512];
        assert_eq!(capture.signal.read(&mut out), None);
        let block = [0.25; 256];
        crate::tests::allocations::COUNT.set(0);
        crate::tests::allocations::ENABLED.set(true);
        let start = std::time::Instant::now();
        for _ in 0..2000 {
            capture.push(&block);
        }
        let elapsed = start.elapsed();
        crate::tests::allocations::ENABLED.set(false);
        assert_eq!(crate::tests::allocations::COUNT.get(), 0);
        println!(
            "Capture 256 samples: mean {:.3} us, 0 allocations",
            elapsed.as_secs_f64() * 1e6 / 2000.0
        );
        assert_eq!(capture.signal.read(&mut out), Some(512000));
        assert!(out.iter().all(|v| *v == 0.25));
        capture.push(&[0.5; 256]);
        capture.signal.read(&mut out).unwrap();
        assert_eq!(
            (out[0], out[255], out[256], out[511]),
            (0.25, 0.25, 0.5, 0.5)
        );
        let signal = capture.signal.clone();
        drop(capture);
        assert_eq!(signal.read(&mut out), None);
    }
}

//! The output of the audio domain to the default audio device (spec 9.3).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering::Acquire, Ordering::Relaxed, Ordering::Release};
use std::thread::sleep;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

/// The samples that wait for the device: 8 blocks, about 43 ms at 48 kHz.
const QUEUE: usize = 2048;

/// A queue of samples without a lock, with one writer and one reader.
struct Ring {
    samples: Box<[AtomicU32]>,
    written: AtomicUsize,
    read: AtomicUsize,
    failed: AtomicBool,
}

impl Ring {
    fn new(len: usize) -> Ring {
        let samples = (0..len).map(|_| AtomicU32::new(0)).collect();
        Ring { samples, written: AtomicUsize::new(0), read: AtomicUsize::new(0), failed: AtomicBool::new(false) }
    }

    fn queued(&self) -> usize {
        self.written.load(Relaxed).wrapping_sub(self.read.load(Acquire))
    }

    /// Adds all of `samples`, or returns false and adds none when the room is too small.
    fn push(&self, samples: &[f32]) -> bool {
        let (len, written) = (self.samples.len(), self.written.load(Relaxed));
        if len - self.queued() < samples.len() {
            return false;
        }
        for (i, sample) in samples.iter().enumerate() {
            self.samples[written.wrapping_add(i) % len].store(sample.to_bits(), Relaxed);
        }
        self.written.store(written.wrapping_add(samples.len()), Release);
        true
    }

    /// Writes one sample to each channel of each frame of `out`. An empty queue gives silence.
    fn fill(&self, out: &mut [f32], channels: usize) {
        let (written, mut read) = (self.written.load(Acquire), self.read.load(Relaxed));
        for frame in out.chunks_mut(channels) {
            let mut sample = 0.0;
            if read != written {
                sample = f32::from_bits(self.samples[read % self.samples.len()].load(Relaxed));
                read = read.wrapping_add(1);
            }
            frame.fill(sample);
        }
        self.read.store(read, Release);
    }
}

/// An open stream to the default output device. The sound is mono on all channels.
pub struct Device {
    ring: Arc<Ring>,
    _stream: cpal::Stream,
}

impl Device {
    /// Opens the default output device at `rate` samples a second, with f32 samples. Returns
    /// `None` when there is no device or the device has no such format.
    pub fn open(rate: u32) -> Option<Device> {
        let device = cpal::default_host().default_output_device()?;
        let config = (device.supported_output_configs().ok()?)
            .filter(|c| c.sample_format() == cpal::SampleFormat::F32)
            .find_map(|c| c.try_with_sample_rate(rate))?;
        let channels = usize::from(config.channels());
        let ring = Arc::new(Ring::new(QUEUE));
        let (reader, failer) = (ring.clone(), ring.clone());
        let stream = device
            .build_output_stream(
                &config.config(),
                move |out: &mut [f32], _: &cpal::OutputCallbackInfo| reader.fill(out, channels),
                move |_| failer.failed.store(true, Relaxed),
                None,
            )
            .ok()?;
        stream.play().ok()?;
        Some(Device { ring, _stream: stream })
    }

    /// Queues `samples` and waits while the queue is full, so the device paces the blocks.
    /// Returns false when the device failed.
    pub fn play(&self, samples: &[f32]) -> bool {
        while !self.ring.push(samples) {
            if self.ring.failed.load(Relaxed) {
                return false;
            }
            sleep(Duration::from_millis(1));
        }
        true
    }

    /// Waits until the device played the queued samples.
    pub fn drain(&self) {
        while self.ring.queued() > 0 && !self.ring.failed.load(Relaxed) {
            sleep(Duration::from_millis(1));
        }
        // shortcut: a fixed wait for the buffer of the device, read its latency if a sound gets cut.
        sleep(Duration::from_millis(50));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ring_keeps_the_order_refuses_an_overflow_and_gives_silence_when_empty() {
        let (ring, mut out) = (Ring::new(4), [9.0; 6]);
        assert!(ring.push(&[1.0, 2.0, 3.0]) && !ring.push(&[4.0, 5.0]));
        ring.fill(&mut out[..4], 2);
        assert_eq!(out[..4], [1.0, 1.0, 2.0, 2.0]);
        assert!(ring.push(&[4.0, 5.0, 6.0]));
        ring.fill(&mut out, 1);
        assert_eq!((out, ring.queued()), ([3.0, 4.0, 5.0, 6.0, 0.0, 0.0], 0));
    }
}

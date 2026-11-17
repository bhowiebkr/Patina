//! The output of the audio domain to the default audio device (spec 9.3).

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering::Acquire, Ordering::Relaxed, Ordering::Release};
use std::thread::sleep;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use crate::clock::{self, Clock};

/// The samples that wait for the device: 8 blocks, about 43 ms at 48 kHz.
const QUEUE: usize = 2048;

/// A queue of samples without a lock, with one writer and one reader.
struct Ring {
    samples: Box<[AtomicU32]>,
    written: AtomicUsize,
    read: AtomicUsize,
    failed: AtomicBool,
    /// The time in microseconds from the last callback of the device to the end of its sound.
    tail: AtomicU32,
}

impl Ring {
    fn new(len: usize) -> Ring {
        let samples = (0..len).map(|_| AtomicU32::new(0)).collect();
        Ring { samples, written: AtomicUsize::new(0), read: AtomicUsize::new(0), failed: AtomicBool::new(false), tail: AtomicU32::new(0) }
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
                move |out: &mut [f32], info: &cpal::OutputCallbackInfo| {
                    let at = info.timestamp();
                    reader.tail.store(tail(at.playback.duration_since(&at.callback), out.len() / channels, rate), Relaxed);
                    reader.fill(out, channels);
                },
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
        sleep(Duration::from_micros(self.ring.tail.load(Relaxed).into()));
    }
}

/// The time in microseconds from a callback of the device to the end of the sound of its
/// `frames`: the latency of the device and the time of the frames at `rate`. At most 1 s, so a
/// device that reports a bad latency does not stop the end of a run.
fn tail(latency: Option<Duration>, frames: usize, rate: u32) -> u32 {
    let play = Duration::from_secs(frames as u64) / rate;
    latency.unwrap_or_default().saturating_add(play).min(Duration::from_secs(1)).as_micros() as u32
}

/// Paces the blocks of the audio domain: the timer until a node plays, then the device (9.3).
pub struct Pace {
    clock: Clock,
    device: Option<Device>,
    opened: bool,
}

impl Default for Pace {
    fn default() -> Pace {
        Pace { clock: Clock::new(clock::BLOCK), device: None, opened: false }
    }
}

impl Pace {
    /// Waits for the time of the next block. With a device, `play` waits instead.
    pub fn wait(&mut self) {
        if self.device.is_none() {
            self.clock.wait();
        }
    }

    /// Sends the sound of a block to the device, and opens the device at the first sound.
    /// Returns a notice when the device opens, is not there or stops.
    pub fn play(&mut self, sound: Option<&[f32]>) -> Option<Notice> {
        let sound = sound?;
        let mut notice = None;
        if !self.opened {
            self.opened = true;
            self.device = Device::open(clock::SAMPLE_RATE);
            notice = Some(if self.device.is_some() { Notice::Playing } else { Notice::NoDevice });
        }
        if self.device.as_ref().is_some_and(|d| !d.play(sound)) {
            (self.device, self.clock) = (None, Clock::new(clock::BLOCK));
            notice = Some(Notice::Stopped);
        }
        notice
    }

    /// Waits until the device played its queue.
    pub fn drain(self) {
        if let Some(device) = self.device {
            device.drain();
        }
    }
}

/// Gives the calling thread a high priority, for the audio thread (9.2). Returns false when the
/// system refuses it.
#[cfg(windows)]
pub fn raise_priority() -> bool {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentThread() -> *mut std::ffi::c_void;
        fn SetThreadPriority(thread: *mut std::ffi::c_void, priority: i32) -> i32;
    }
    const THREAD_PRIORITY_HIGHEST: i32 = 2;
    // SAFETY: the pseudo handle of the current thread is always valid on this thread.
    unsafe { SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_HIGHEST) != 0 }
}

// shortcut: only Windows raises the priority, Linux needs CAP_SYS_NICE or rtkit; add it when a user there hears gaps.
#[cfg(not(windows))]
pub fn raise_priority() -> bool {
    false
}

/// A change of the audio device that the user sees (9.3).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Notice {
    Playing,
    NoDevice,
    Stopped,
}

impl Notice {
    /// True when the sound does not play.
    pub fn warning(self) -> bool {
        self != Notice::Playing
    }
}

impl fmt::Display for Notice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Notice::Playing => write!(f, "Playing the sound on the default audio device"),
            Notice::NoDevice => write!(f, "no audio device takes {} Hz, so the sound does not play", clock::SAMPLE_RATE),
            Notice::Stopped => write!(f, "the audio device stopped"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_audio_thread_gets_a_high_priority_on_windows() {
        assert_eq!(std::thread::spawn(raise_priority).join().unwrap(), cfg!(windows));
    }

    #[test]
    fn the_tail_adds_the_latency_to_the_frames_and_stops_at_one_second() {
        let ms = |ms| Some(Duration::from_millis(ms));
        assert_eq!(tail(ms(10), 480, 48_000), 20_000);
        assert_eq!(tail(None, 480, 48_000), 10_000);
        assert_eq!(tail(ms(5_000), 480, 48_000), 1_000_000);
    }

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

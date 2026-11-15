//! The 60 Hz lane-state feed: packed bits, sent by a Tauri channel or a JSON event (15.6).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{AppHandle, Emitter};

use crate::graph::Rng;

/// The frame id (u32 LE) and the send time in Unix ms (f64 LE) come before the bits.
pub const HEADER_BYTES: usize = 12;
const PERIOD: Duration = Duration::from_nanos(16_666_667);

pub enum Sink {
    Channel(Channel<InvokeResponseBody>),
    Event,
}

pub struct Feed {
    pub lane_count: u32,
    generation: AtomicU64,
    /// The time of each send call in microseconds, since the last `take_send_times`.
    send_us: Mutex<Vec<u32>>,
}

impl Feed {
    pub fn new(lane_count: u32) -> Self {
        Self { lane_count, generation: AtomicU64::new(0), send_us: Mutex::new(Vec::new()) }
    }

    pub fn frame_bytes(&self) -> usize {
        HEADER_BYTES + self.lane_count.div_ceil(8) as usize
    }

    pub fn take_send_times(&self) -> Vec<u32> {
        std::mem::take(&mut *self.send_us.lock().unwrap())
    }

    /// Stops the feed thread. A send into a page that is gone does not fail, so it cannot stop alone.
    pub fn stop(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
    }

    /// Starts a new feed thread. The thread of an earlier page load stops.
    pub fn start(self: &Arc<Self>, app: AppHandle, sink: Sink) {
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let feed = Arc::clone(self);
        thread::spawn(move || feed.run(generation, &app, &sink));
    }

    fn run(&self, generation: u64, app: &AppHandle, sink: &Sink) {
        let mut rng = Rng::new(generation);
        let bytes = self.frame_bytes();
        let mut next = Instant::now();
        let mut id = 0u32;
        while self.generation.load(Ordering::SeqCst) == generation {
            let frame = make_frame(&mut rng, id, unix_ms(), bytes);
            let start = Instant::now();
            // shortcut: Tauri takes the frame by value, so each send allocates; a pooled path needs shared memory (15.6).
            let sent = match sink {
                Sink::Channel(channel) => channel.send(InvokeResponseBody::Raw(frame)).is_ok(),
                Sink::Event => app.emit("lanes", &frame).is_ok(),
            };
            let us = start.elapsed().as_micros().min(u32::MAX as u128) as u32;
            self.send_us.lock().unwrap().push(us);
            if !sent {
                return;
            }
            id = id.wrapping_add(1);
            next += PERIOD;
            let now = Instant::now();
            if next > now {
                thread::sleep(next - now);
            } else if now - next > PERIOD {
                next = now;
            }
        }
    }
}

fn unix_ms() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64() * 1000.0)
}

/// All bits are random each frame: about half of the lanes change, the worst case.
fn make_frame(rng: &mut Rng, id: u32, ms: f64, bytes: usize) -> Vec<u8> {
    let mut frame = Vec::with_capacity(bytes + 8);
    frame.extend_from_slice(&id.to_le_bytes());
    frame.extend_from_slice(&ms.to_le_bytes());
    while frame.len() < bytes {
        frame.extend_from_slice(&rng.next().to_le_bytes());
    }
    frame.truncate(bytes);
    frame
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_has_the_header_and_the_lane_bits() {
        let feed = Feed::new(4481);
        assert_eq!(feed.frame_bytes(), HEADER_BYTES + 561);
        let frame = make_frame(&mut Rng::new(3), 7, 1.5, feed.frame_bytes());
        assert_eq!(frame.len(), feed.frame_bytes());
        assert_eq!(u32::from_le_bytes(frame[0..4].try_into().unwrap()), 7);
        assert_eq!(f64::from_le_bytes(frame[4..12].try_into().unwrap()), 1.5);
    }
}

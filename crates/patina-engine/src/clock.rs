//! The clock of the frame domain (spec 9.1).

use std::time::{Duration, Instant};

pub use patina_sdk::audio::SAMPLE_RATE;

/// The frame period at 60 FPS.
pub const FRAME: Duration = Duration::from_nanos(16_666_667);

/// The period of one block of the audio domain (9.3).
pub const BLOCK: Duration =
    Duration::from_nanos(1_000_000_000 * patina_sdk::audio::BLOCK as u64 / patina_sdk::audio::SAMPLE_RATE as u64);

/// Ticks at a fixed period. A late frame skips the ticks that it missed, so frames never queue.
pub struct Clock {
    next: Instant,
    period: Duration,
}

impl Clock {
    pub fn new(period: Duration) -> Clock {
        Clock { next: Instant::now() + period, period }
    }

    /// Waits for the next tick.
    pub fn wait(&mut self) {
        let now = Instant::now();
        match self.next.checked_duration_since(now) {
            Some(rest) => std::thread::sleep(rest),
            // Late: start now. The next tick is the first tick after now.
            None => {
                let late = now.duration_since(self.next).as_nanos() % self.period.as_nanos();
                self.next = now - Duration::from_nanos(late as u64);
            }
        }
        self.next += self.period;
    }
}

/// True when frame `frame` starts at or before the start of audio block `block`. A run with
/// blocks and frames on one thread runs a frame before the first block after its time (9.3).
pub fn frame_due(frame: u64, block: u64) -> bool {
    u128::from(frame) * FRAME.as_nanos() <= u128::from(block) * BLOCK.as_nanos()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_runs_before_the_first_block_after_its_time() {
        assert!(frame_due(0, 0) && frame_due(1, 4) && !frame_due(1, 3) && frame_due(2, 7) && !frame_due(2, 6));
    }

    #[test]
    fn a_late_frame_skips_the_missed_ticks() {
        let period = Duration::from_millis(10);
        let start = Instant::now();
        let mut clock = Clock { next: start - period * 5 / 2, period };
        clock.wait();
        let tick = clock.next;
        assert!(tick > start && tick <= Instant::now() + period);
        clock.wait();
        assert!(Instant::now() >= tick && clock.next == tick + period);
    }
}

//! The clock of the frame domain (spec 9.1).

use std::time::{Duration, Instant};

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

#[cfg(test)]
mod tests {
    use super::*;

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

//! The clock of the frame domain (spec 9.1).

use std::time::{Duration, Instant};

pub use patina_sdk::audio::SAMPLE_RATE;

/// The frame period at 60 FPS, the period of the clock of the frame domain.
pub const FRAME: Duration = Duration::from_nanos(16_666_667);

/// The frame rates that a project can have (6.4), and the rate of a project with no rate.
pub const RATES: std::ops::RangeInclusive<u32> = 1..=1000;
pub const RATE: u32 = 60;

/// The number of frames that run at tick `tick` (from 1) of the 60 FPS clock, for `rate` frames
/// each second (9.1). The first tick runs a frame. A rate of 600 gives 10 frames at each tick, and
/// 30 gives 1 frame at each second tick.
pub fn frames_at(tick: u64, rate: u32) -> u64 {
    let done = |tick: u64| (tick * u64::from(rate)).div_ceil(60);
    done(tick) - done(tick.saturating_sub(1))
}

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

/// True when frame `frame` starts at or before the start of audio block `block`, at `rate` frames
/// each second. A run with blocks and frames on one thread runs a frame before the first block
/// after its time (9.3).
pub fn frame_due(frame: u64, block: u64, rate: u32) -> bool {
    u128::from(frame) * 1_000_000_000 <= u128::from(block) * BLOCK.as_nanos() * u128::from(rate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_runs_before_the_first_block_after_its_time() {
        let due = |frame, block| frame_due(frame, block, RATE);
        assert!(due(0, 0) && due(1, 4) && !due(1, 3) && due(2, 7) && !due(2, 6));
        assert!(frame_due(5, 4, 4 * RATE) && !frame_due(6, 4, 4 * RATE));
    }

    #[test]
    fn a_rate_gives_its_frames_at_the_60_fps_ticks() {
        let sum = |rate| (1..=60).map(|tick| frames_at(tick, rate)).collect::<Vec<_>>();
        assert_eq!(sum(60), [1; 60]);
        assert_eq!(sum(600), [10; 60]);
        assert_eq!(sum(30)[..4], [1, 0, 1, 0]);
        for rate in [1, 7, 61, 1000] {
            assert_eq!(sum(rate).iter().sum::<u64>(), u64::from(rate));
        }
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

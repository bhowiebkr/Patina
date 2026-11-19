use std::sync::atomic::{AtomicU32, AtomicU64, Ordering::Relaxed};

/// Transient input shared by one sequencer and its keyboard views.
#[derive(Debug, Default)]
pub struct LiveKeys {
    state: AtomicU64,
    pulse: AtomicU32,
}
impl LiveKeys {
    pub fn set(&self, note: Option<u8>, velocity: u8) {
        let old = self.state.load(Relaxed);
        let serial = (old >> 16).wrapping_add(1);
        self.state.store(
            (serial << 16)
                | (u64::from(velocity.max(1)) << 8)
                | note.map_or(0, |n| u64::from(n.min(127)) + 1),
            Relaxed,
        );
    }
    pub fn heartbeat(&self) {
        self.pulse.fetch_add(1, Relaxed);
    }
    pub fn note(&self) -> Option<u8> {
        (self.state.load(Relaxed) as u8).checked_sub(1)
    }
    pub(crate) fn read(&self) -> (u64, u32) {
        (self.state.load(Relaxed), self.pulse.load(Relaxed))
    }
}

/// Releases input after 0.5 seconds without a visible keyboard's heartbeat.
#[derive(Default)]
pub(crate) struct KeyLease {
    pulse: u32,
    blocks: u32,
    serial: u64,
}
impl KeyLease {
    pub fn read(&mut self, keys: &LiveKeys) -> Option<(u8, f32, bool)> {
        let (state, pulse) = keys.read();
        if pulse != self.pulse {
            self.blocks = 0;
            self.pulse = pulse;
        } else {
            self.blocks = self.blocks.saturating_add(1);
        }
        let serial = state >> 16;
        let strike = serial != self.serial;
        self.serial = serial;
        let encoded = (state & 255) as u8;
        if self.blocks >= 94 {
            let _ = keys
                .state
                .compare_exchange(state, state & !255, Relaxed, Relaxed);
        }
        (encoded > 0 && self.blocks < 94)
            .then(|| (encoded - 1, ((state >> 8) & 255) as f32 / 127.0, strike))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keyboard_retriggers_and_expires_without_ui_calls() {
        let keys = LiveKeys::default();
        let mut lease = KeyLease::default();
        keys.set(Some(60), 64);
        keys.heartbeat();
        assert_eq!(lease.read(&keys), Some((60, 64.0 / 127.0, true)));
        assert_eq!(lease.read(&keys), Some((60, 64.0 / 127.0, false)));
        for _ in 0..94 {
            lease.read(&keys);
        }
        assert_eq!(lease.read(&keys), None);
        keys.set(None, 127);
        keys.heartbeat();
        assert_eq!(lease.read(&keys), None);
    }
}

//! A separate voice for preset audition. It never changes the saved patch.
use super::{Clock, Color, Contour, Echo, Envelope, Filter, Oscillator, Sequence, Space, Tone};
use patina_sdk::AudioBlock;
use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicBool, AtomicU32, Ordering::Relaxed},
};

pub struct Preview {
    values: [AtomicU32; 15],
    pub active: AtomicBool,
    pulse: AtomicU32,
    level: AtomicU32,
    alive: AtomicBool,
}
impl Preview {
    pub fn start(&self, values: [f32; 15], level: f32) {
        for (target, value) in self.values.iter().zip(values) {
            target.store(value.to_bits(), Relaxed);
        }
        self.level.store(level.clamp(0.0, 0.5).to_bits(), Relaxed);
        self.active.store(true, Relaxed);
        self.heartbeat();
    }
    pub fn stop(&self) {
        self.active.store(false, Relaxed);
    }
    pub fn heartbeat(&self) {
        self.pulse.fetch_add(1, Relaxed);
    }
    pub fn level(&self, value: f32) {
        self.level.store(value.clamp(0.0, 0.5).to_bits(), Relaxed);
    }
}
#[derive(Default)]
struct Registry(Mutex<Vec<Weak<Preview>>>);
pub fn outputs() -> Vec<Arc<Preview>> {
    let mut entries = patina_sdk::shared(Registry::default)
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    entries.retain(|e| e.upgrade().is_some_and(|p| p.alive.load(Relaxed)));
    entries.iter().filter_map(Weak::upgrade).collect()
}
pub struct Audition {
    control: Arc<Preview>,
    clock: Clock,
    sequence: Sequence,
    oscillator: Oscillator,
    envelope: Envelope,
    filter: Filter,
    echo: Echo,
    pulse: u32,
    blocks: u32,
    fade: f32,
}
impl Default for Audition {
    fn default() -> Self {
        let control = Arc::new(Preview {
            values: [const { AtomicU32::new(0) }; 15],
            active: AtomicBool::new(false),
            pulse: AtomicU32::new(0),
            level: AtomicU32::new(0.15_f32.to_bits()),
            alive: AtomicBool::new(true),
        });
        patina_sdk::shared(Registry::default)
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(Arc::downgrade(&control));
        Self {
            control,
            clock: Clock::default(),
            sequence: Sequence::default(),
            oscillator: Oscillator::default(),
            envelope: Envelope::default(),
            filter: Filter::default(),
            echo: Echo::default(),
            pulse: 0,
            blocks: 0,
            fade: 0.0,
        }
    }
}
impl Audition {
    pub fn mix(&mut self, main: &mut AudioBlock) {
        let pulse = self.control.pulse.load(Relaxed);
        if pulse != self.pulse {
            self.blocks = 0;
            self.pulse = pulse;
        } else {
            self.blocks = self.blocks.saturating_add(1);
        }
        let active = self.control.active.load(Relaxed) && self.blocks < 94;
        if self.blocks >= 94 {
            self.control.stop();
        }
        if !active && self.fade < 0.0001 {
            return;
        }
        let v: [f32; 15] =
            std::array::from_fn(|i| f32::from_bits(self.control.values[i].load(Relaxed)));
        let tone = Tone {
            shape: v[0] as u8,
            detune: v[1],
            sub: v[2],
            octave: v[3] as i8,
        };
        let contour = Contour {
            attack: v[4],
            decay: v[5],
            sustain: v[6],
            release: v[7],
        };
        let color = Color {
            cutoff: v[8],
            resonance: v[9],
            motion: v[10],
            rate: v[11],
        };
        let space = Space {
            seconds: v[12],
            feedback: v[13],
            mix: v[14],
        };
        let notes = self.clock.process(&self.sequence);
        let sound = self.oscillator.process(&notes, &tone);
        let sound = self.envelope.process(&sound, &notes, &contour);
        let sound = self.filter.process(&sound, &color);
        let sound = self.echo.process(&sound, &space);
        let gain = f32::from_bits(self.control.level.load(Relaxed));
        for (i, sample) in main.iter_mut().enumerate() {
            self.fade += (f32::from(active) - self.fade) * 0.003;
            *sample = *sample * (1.0 - self.fade) + (sound[i] * gain).tanh() * 0.7 * self.fade;
        }
    }
}

impl Drop for Audition {
    fn drop(&mut self) {
        self.control.alive.store(false, Relaxed);
        self.control.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn audition_is_audible_allocates_nothing_warm_and_expires() {
        let mut voice = Audition::default();
        voice.control.start(
            [
                1.0, 9.0, 0.28, 0.0, 0.008, 0.2, 0.35, 0.24, 1600.0, 0.4, 1.2, 0.18, 0.402, 0.42,
                0.3,
            ],
            0.2,
        );
        let mut block = AudioBlock::silence();
        for _ in 0..100 {
            voice.control.heartbeat();
            voice.mix(&mut block);
        }
        crate::tests::allocations::COUNT.set(0);
        crate::tests::allocations::ENABLED.set(true);
        let mut peak = 0.0_f32;
        for _ in 0..1000 {
            block.fill(0.0);
            voice.control.heartbeat();
            voice.mix(&mut block);
            peak = peak.max(block.peak());
        }
        crate::tests::allocations::ENABLED.set(false);
        assert_eq!(crate::tests::allocations::COUNT.get(), 0);
        assert!(peak > 0.01 && peak < 0.7);
        for _ in 0..160 {
            block.fill(0.0);
            voice.mix(&mut block);
        }
        assert!(!voice.control.active.load(Relaxed));
        assert!(block.peak() < 0.0001);
    }
}

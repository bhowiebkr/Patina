//! Blocks of audio samples (spec 9.3). A block comes from a pool of the thread, so a node of the
//! audio domain makes and returns blocks without an allocation.

use std::cell::RefCell;
use std::fmt::{self, Write as _};
use std::ops::{Deref, DerefMut};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};

/// The samples in one block (9.3).
pub const BLOCK: usize = 256;
/// The samples in one second.
pub const SAMPLE_RATE: u32 = 48_000;

type Samples = Box<[f32; BLOCK]>;

/// The most free blocks that the pool of one thread keeps.
const POOL: usize = 64;

thread_local! {
    static FREE: RefCell<Vec<Samples>> = RefCell::new(Vec::with_capacity(POOL));
}

/// One block of mono samples. A new block is silent.
pub struct AudioBlock(Option<Samples>);

impl AudioBlock {
    /// A silent block from the pool. Only the first blocks of a thread allocate.
    pub fn silence() -> AudioBlock {
        let free = FREE.try_with(|free| free.borrow_mut().pop()).ok().flatten();
        AudioBlock(Some(free.map_or_else(|| Box::new([0.0; BLOCK]), |mut samples| {
            samples.fill(0.0);
            samples
        })))
    }

    /// A block with the sample `f(i)` at each index `i`.
    pub fn from_fn(mut f: impl FnMut(usize) -> f32) -> AudioBlock {
        let mut block = AudioBlock::silence();
        block.iter_mut().enumerate().for_each(|(i, s)| *s = f(i));
        block
    }

    /// A block with `f` applied to each sample of this block.
    pub fn map(&self, mut f: impl FnMut(f32) -> f32) -> AudioBlock {
        AudioBlock::from_fn(|i| f(self[i]))
    }

    /// The largest absolute sample.
    pub fn peak(&self) -> f32 {
        self.iter().fold(0.0, |peak, s| peak.max(s.abs()))
    }
}

impl Default for AudioBlock {
    fn default() -> AudioBlock {
        AudioBlock::silence()
    }
}

impl Clone for AudioBlock {
    fn clone(&self) -> AudioBlock {
        AudioBlock::from_fn(|i| self[i])
    }

    fn clone_from(&mut self, source: &AudioBlock) {
        self.copy_from_slice(source);
    }
}

impl Drop for AudioBlock {
    fn drop(&mut self) {
        if let Some(samples) = self.0.take() {
            // A full pool or a thread at its end drops the block.
            let _ = FREE.try_with(|free| {
                let mut free = free.borrow_mut();
                if free.len() < POOL {
                    free.push(samples);
                }
            });
        }
    }
}

impl Deref for AudioBlock {
    type Target = [f32];

    fn deref(&self) -> &[f32] {
        self.0.as_deref().map_or(&[], |samples| samples)
    }
}

impl DerefMut for AudioBlock {
    fn deref_mut(&mut self) -> &mut [f32] {
        self.0.as_deref_mut().map_or(&mut [], |samples| samples)
    }
}

impl fmt::Debug for AudioBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AudioBlock(peak {:.3})", self.peak())
    }
}

impl crate::Viewer for AudioBlock {
    fn text(&self, out: &mut crate::Text<'_>) -> fmt::Result {
        write!(out, "{self:?}")
    }

    fn wave(&self) -> &[f32] {
        self
    }
}

/// The sum of the blocks that the audio nodes of a project play in one block. The engine sends it
/// to the audio device. Only the audio domain writes it, so the atomics never wait.
#[repr(C)]
pub struct Mix {
    samples: [AtomicU32; BLOCK],
    played: AtomicBool,
}

impl Default for Mix {
    fn default() -> Mix {
        Mix { samples: [const { AtomicU32::new(0) }; BLOCK], played: AtomicBool::new(false) }
    }
}

impl Mix {
    /// Adds `block` to the mix.
    pub fn add(&self, block: &AudioBlock) {
        for (sum, sample) in self.samples.iter().zip(block.iter()) {
            sum.store((f32::from_bits(sum.load(Relaxed)) + sample).to_bits(), Relaxed);
        }
        self.played.store(true, Relaxed);
    }

    /// Moves the mix into `out` and starts a new silent mix.
    pub fn take(&self, out: &mut [f32; BLOCK]) {
        for (out, sum) in out.iter_mut().zip(&self.samples) {
            *out = f32::from_bits(sum.swap(0, Relaxed));
        }
    }

    /// True after the first block that a node played.
    pub fn played(&self) -> bool {
        self.played.load(Relaxed)
    }
}

/// Sends `block` to the audio device, with the blocks of the other nodes that play. Call it from
/// a node of the audio domain (9.3).
pub fn play(block: &AudioBlock) {
    static MIX: OnceLock<&'static Mix> = OnceLock::new();
    MIX.get_or_init(|| crate::glue::shared(Mix::default)).add(block);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dropped_block_goes_back_to_the_pool_and_comes_back_silent() {
        let block = AudioBlock::from_fn(|i| i as f32);
        let address = block.as_ptr();
        assert_eq!((block.len(), block[3], block.peak()), (BLOCK, 3.0, 255.0));
        drop(block);
        let again = AudioBlock::silence();
        assert_eq!((again.as_ptr(), again.peak()), (address, 0.0));
        let mut copy = again.map(|s| s + 0.5);
        copy.clone_from(&again);
        assert_eq!(copy.peak(), 0.0);
    }

    #[test]
    fn the_mix_adds_the_blocks_and_starts_again_after_a_take() {
        let (mix, mut out) = (Mix::default(), [1.0; BLOCK]);
        assert!(!mix.played());
        mix.add(&AudioBlock::from_fn(|i| i as f32));
        mix.add(&AudioBlock::from_fn(|_| 0.5));
        mix.take(&mut out);
        assert_eq!((out[0], out[3], mix.played()), (0.5, 3.5, true));
        mix.take(&mut out);
        assert_eq!(out, [0.0; BLOCK]);
    }
}

//! The latest-value wire without a lock (spec 9.1, 9.3): a buffer of three values. The audio
//! thread reads and writes it, and never waits, allocates or drops a value of another thread.

use std::cell::UnsafeCell;
use std::ffi::c_void;
use std::ops::Deref;
use std::sync::atomic::{AtomicU8, Ordering::AcqRel, Ordering::Relaxed};
use std::sync::{Arc, OnceLock};

use super::{Moved, Shared};

/// Set in `Triple::middle` when the writer put a value in the middle after the last fetch.
const FRESH: u8 = 4;

/// Three values. The writer owns one, the reader owns one, and the third is in the middle. A swap
/// with the middle gives a value to the other side.
pub struct Triple<T> {
    slots: [UnsafeCell<Option<T>>; 3],
    middle: AtomicU8,
    /// Only the writer uses it.
    back: AtomicU8,
    /// Only the reader uses it.
    front: AtomicU8,
}

// SAFETY: each slot has one owner at a time: the writer, the reader or the middle. The swaps of
// `middle` move the ownership with acquire and release order.
unsafe impl<T: Send> Send for Triple<T> {}
// SAFETY: the same as for `Send`. One thread writes and one thread reads at a time.
unsafe impl<T: Send> Sync for Triple<T> {}

impl<T> Triple<T> {
    pub(super) fn new() -> Triple<T> {
        let slots = [UnsafeCell::new(None), UnsafeCell::new(None), UnsafeCell::new(None)];
        Triple { slots, middle: AtomicU8::new(1), back: AtomicU8::new(0), front: AtomicU8::new(2) }
    }

    /// Puts a copy of `value` in the middle for the reader. One thread writes at a time.
    pub(super) fn publish(&self, value: &Option<T>)
    where
        T: Clone,
    {
        let back = self.back.load(Relaxed);
        // SAFETY: only the writer uses the back slot.
        let slot = unsafe { &mut *self.slots[usize::from(back)].get() };
        match (slot.as_mut(), value) {
            // A copy into the old value keeps its memory, so most types do not allocate.
            (Some(old), Some(new)) => old.clone_from(new),
            _ => slot.clone_from(value),
        }
        let old = self.middle.swap(back | FRESH, AcqRel);
        self.back.store(old & !FRESH, Relaxed);
    }

    /// Swaps the newest value into `own` when the writer gave one after the last fetch. The old
    /// value of `own` goes back to the writer, which drops or reuses it. One thread reads at a time.
    pub(super) fn fetch(&self, own: &mut Option<T>) {
        if self.middle.load(Relaxed) & FRESH == 0 {
            return;
        }
        let old = self.middle.swap(self.front.load(Relaxed), AcqRel) & !FRESH;
        self.front.store(old, Relaxed);
        // SAFETY: the swap gave the slot to the reader.
        std::mem::swap(own, unsafe { &mut *self.slots[usize::from(old)].get() });
    }
}

/// The most clusters of other domains that read one value.
const READERS: usize = 3;

/// A value that a cluster of another domain reads through a `Triple`: a wire into or out of the
/// audio domain (9.3). The nodes of this cluster use its own value.
pub struct Latest<T> {
    own: Shared<Option<T>>,
    to: [OnceLock<Arc<Triple<T>>>; READERS],
}

impl<T> Latest<T> {
    #[allow(clippy::new_without_default, reason = "the generated code calls only `new`")]
    pub fn new() -> Latest<T> {
        Latest { own: Shared::new(None), to: [OnceLock::new(), OnceLock::new(), OnceLock::new()] }
    }

    /// Gives a copy of the value to each reader. The generated code calls it after the node.
    pub fn publish(&self)
    where
        T: Clone,
    {
        let value = self.own.read();
        for triple in self.to.iter().filter_map(OnceLock::get) {
            triple.publish(&value);
        }
    }
}

impl<T> Deref for Latest<T> {
    type Target = Shared<Option<T>>;

    fn deref(&self) -> &Shared<Option<T>> {
        &self.own
    }
}

/// The reader side of a `Latest` in the cluster of another domain. It is empty until the engine
/// links it.
pub struct LatestLink<T> {
    own: Shared<Option<T>>,
    link: OnceLock<Arc<Triple<T>>>,
}

impl<T> LatestLink<T> {
    #[allow(clippy::new_without_default, reason = "the generated code calls only `new`")]
    pub fn new() -> LatestLink<T> {
        LatestLink { own: Shared::new(None), link: OnceLock::new() }
    }

    /// Gives Rust the type of the value. The generated code calls it in a block that never runs.
    pub fn witness(&self, _value: T) {}

    /// Gets the newest value. The generated code calls it at the start of a run, before the tasks.
    pub fn fetch(&self) {
        if let Some(triple) = self.link.get() {
            triple.fetch(&mut self.own.write());
        }
    }
}

impl<T> Deref for LatestLink<T> {
    type Target = Shared<Option<T>>;

    fn deref(&self) -> &Shared<Option<T>> {
        &self.own
    }
}

/// Writes a reference to a new buffer of `value` for `import_latest` in another cluster. False
/// when the value has all its readers.
pub fn export_latest<T>(value: &Latest<T>, out: &mut *mut c_void) -> bool {
    let Some(free) = value.to.iter().find(|to| to.get().is_none()) else { return false };
    let triple = Arc::clone(free.get_or_init(|| Arc::new(Triple::new())));
    *out = Arc::into_raw(triple).cast_mut().cast();
    true
}

/// Links `link` to a buffer from `export_latest`. Returns false when it has a link already.
pub fn import_latest<T>(link: &LatestLink<T>, moved: Moved) -> bool {
    // SAFETY: `moved` came from `export` of the same link in a cluster of the same build, so it
    // is an `Arc` of the same type (see `Moved`). The engine gives it one time.
    let triple = unsafe { Arc::from_raw(moved.0.cast_const().cast::<Triple<T>>()) };
    link.link.set(triple).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reader_gets_the_newest_value_and_the_old_value_goes_back() {
        let triple = Triple::new();
        let mut own = None;
        triple.fetch(&mut own);
        assert_eq!(own, None, "no value yet");
        triple.publish(&Some(vec![1]));
        triple.publish(&Some(vec![2]));
        triple.fetch(&mut own);
        assert_eq!(own, Some(vec![2]));
        triple.fetch(&mut own);
        assert_eq!(own, Some(vec![2]), "no new value keeps the value");
        triple.publish(&Some(vec![3]));
        triple.fetch(&mut own);
        assert_eq!(own, Some(vec![3]));
    }

    #[test]
    fn a_reader_on_another_thread_never_sees_a_torn_value() {
        let triple = Arc::new(Triple::new());
        let writer = Arc::clone(&triple);
        let thread = std::thread::spawn(move || (0..2000u32).for_each(|i| writer.publish(&Some([i; 16]))));
        let (mut own, mut last) = (None, 0);
        while !thread.is_finished() || last < 1999 {
            triple.fetch(&mut own);
            if let Some(v) = own {
                assert!(v.iter().all(|&x| x == v[0]) && v[0] >= last);
                last = v[0];
            }
        }
        thread.join().unwrap();
    }
}

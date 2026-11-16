//! The typed outputs of a view (15.8 rule 3). A cluster keeps a copy of each held value that a
//! view reads, and the view reads the copy between two runs. `Any` gives the type check: the
//! clusters and the views of a project link one build of each type crate (5.2).

use std::any::Any;
use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::{Mutex, PoisonError};

use super::{Cluster, Cmd, Probe, Reporter};

/// A kept copy of a held value, as `Box<dyn Any + Send>`.
type Copy = Option<Box<dyn Any + Send>>;

/// The kept copies of the held values of a cluster, in the order of `Plan::held`. A copy is made
/// only after a view asked for it.
pub struct Keeps(Box<[(AtomicBool, Mutex<Copy>)]>);

impl Keeps {
    pub fn new(count: usize) -> Keeps {
        Keeps((0..count).map(|_| (AtomicBool::new(false), Mutex::new(None))).collect())
    }

    /// Keeps a copy of `value` when a view asked for held value `index`. The copy uses the
    /// buffers of the last copy again.
    pub fn keep<T: Clone + Send + 'static>(&self, index: u32, value: &T) {
        let Some((on, copy)) = self.0.get(index as usize) else { return };
        if !on.load(Relaxed) {
            return;
        }
        let mut copy = copy.lock().unwrap_or_else(PoisonError::into_inner);
        let caught = catch_unwind(AssertUnwindSafe(|| match copy.as_mut().and_then(|c| c.downcast_mut::<T>()) {
            Some(old) => old.clone_from(value),
            None => *copy = Some(Box::new(value.clone())),
        }));
        if caught.is_err() {
            *copy = None;
        }
    }

    /// The copy of held value `index` as a pointer to its `Box<dyn Any + Send>`, or null. The
    /// first read only asks for copies, so it gives null.
    pub fn read(&self, index: u32) -> *mut c_void {
        let Some((on, copy)) = self.0.get(index as usize) else { return std::ptr::null_mut() };
        if !on.swap(true, Relaxed) {
            return std::ptr::null_mut();
        }
        let copy = copy.lock().unwrap_or_else(PoisonError::into_inner);
        copy.as_ref().map_or(std::ptr::null_mut(), |c| std::ptr::from_ref(c).cast_mut().cast())
    }
}

/// The generated code calls `(&&Probe(v)).keep(keeps, index)`, so Rust uses `CloneKeep` when the
/// type has `Clone` and `Send`, and `NoKeep` if not. A view cannot read a type without them.
pub trait CloneKeep {
    fn keep(&self, keeps: &Keeps, index: u32);
}

impl<T: Clone + Send + 'static> CloneKeep for &Probe<'_, T> {
    fn keep(&self, keeps: &Keeps, index: u32) {
        keeps.keep(index, self.0);
    }
}

pub trait NoKeep {
    fn keep(&self, keeps: &Keeps, index: u32);
}

impl<T> NoKeep for Probe<'_, T> {
    fn keep(&self, _: &Keeps, _: u32) {}
}

/// The body of `patina_cluster_read`: the kept copy of held value `index` (see `Keeps::read`).
///
/// # Safety
///
/// The same as for `ui`. The pointer stays valid until the next run or the free of the cluster.
pub unsafe fn read(cluster: *const Cluster, index: u32, rep: *const Reporter) -> *mut c_void {
    // SAFETY: the caller keeps the rules of this function.
    let (cluster, rep) = unsafe { (&*cluster, &*rep) };
    let mut out = std::ptr::null_mut();
    let _ = catch_unwind(AssertUnwindSafe(|| (cluster.f)(Cmd::Read(index, &mut out), rep)));
    out
}

/// The engine function that gives a view the kept copy of output `key` of node `id` (15.8), or
/// null. The texts are UTF-8.
pub type ReadFn = unsafe extern "C" fn(ctx: *mut c_void, id: *const u8, id_len: usize, key: *const u8, key_len: usize) -> *const c_void;

/// The engine side of `Graph::value`. It stays valid during a call of `ui`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Reader {
    pub ctx: *mut c_void,
    pub read: ReadFn,
}

impl Reader {
    /// The kept copy of output `key` of node `id` as a `T`. None when the cluster has no copy
    /// yet, or when the output has another type.
    ///
    /// # Safety
    ///
    /// The reader is valid, and the copy is used only during this call of `ui`.
    pub unsafe fn value<'a, T: 'static>(self, id: &str, key: &str) -> Option<&'a T> {
        // SAFETY: the caller keeps the rules of this function. The texts are valid during the call.
        let copy = unsafe { (self.read)(self.ctx, id.as_ptr(), id.len(), key.as_ptr(), key.len()) };
        // SAFETY: a non-null pointer comes from `Keeps::read`, so it points to a live copy.
        unsafe { copy.cast::<Box<dyn Any + Send>>().as_ref() }?.downcast_ref()
    }
}

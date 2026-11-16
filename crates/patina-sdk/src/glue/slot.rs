//! The link of a base cluster to the slot library (7.1). The base calls each use of the node in
//! the slot through a function pointer.

use std::ffi::c_void;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicPtr, Ordering::Acquire, Ordering::Release};

use super::{Cluster, Cmd, ERROR, NODE, RUN, Reporter, catch_unwind};

/// The function of a slot library that calls one node (`call`).
pub type CallFn = unsafe extern "C" fn(*const c_void, u32, *const *mut c_void, usize, *const Reporter) -> bool;

/// The slot library as the base sees it: its cluster and its `call` function (7.1).
#[repr(C)]
pub struct SlotRef {
    pub state: *const c_void,
    pub call: CallFn,
}

/// The link of a base to the slot library (7.1). It is empty until the engine gives the slot.
pub struct Slot(AtomicPtr<SlotRef>);

impl Slot {
    #[allow(clippy::new_without_default, reason = "the generated code calls only `new`")]
    pub fn new() -> Slot {
        Slot(AtomicPtr::new(std::ptr::null_mut()))
    }

    pub fn set(&self, slot: *const SlotRef) -> bool {
        self.0.store(slot.cast_mut(), Release);
        true
    }

    /// Calls node `node` in the slot. With no slot library, it reports an error.
    pub fn call(&self, node: u32, args: &[*mut c_void], rep: &Reporter) -> bool {
        // SAFETY: the engine keeps the slot valid until it gives the next slot, and it does not
        // give a slot during a run.
        let Some(slot) = (unsafe { self.0.load(Acquire).as_ref() }) else {
            rep.send(node, ERROR, "the node has no build in the slot");
            return false;
        };
        // SAFETY: the slot library made `state` and `call` (`patina_cluster_call`), and the
        // generated base gives the arguments in the order that the slot reads them.
        unsafe { (slot.call)(slot.state, node, args.as_ptr(), args.len(), rep) }
    }
}

/// A pointer to an argument of `Slot::call` that the slot only reads.
pub fn arg<T>(value: &T) -> *mut c_void {
    std::ptr::from_ref(value).cast_mut().cast()
}

/// A pointer to an argument of `Slot::call` that the slot can change.
pub fn arg_mut<T>(value: &mut T) -> *mut c_void {
    std::ptr::from_mut(value).cast()
}

/// Reads an argument of `arg` in the slot.
///
/// # Safety
///
/// `p` came from `arg` or `arg_mut` with a value of type `T` that lives during the call.
pub unsafe fn read_arg<'a, T>(p: *mut c_void) -> &'a T {
    // SAFETY: the caller keeps the rules of this function.
    unsafe { &*p.cast::<T>() }
}

/// Takes the value of an argument of `arg_mut` with an `Option<T>` in the slot.
///
/// # Safety
///
/// `p` came from `arg_mut` with an `Option<T>` that lives during the call.
pub unsafe fn take_arg<T>(p: *mut c_void) -> T {
    // SAFETY: the caller keeps the rules of this function.
    unsafe { (*p.cast::<Option<T>>()).take().expect("the base gives each value one time") }
}

/// Changes an argument of `arg_mut` in the slot.
///
/// # Safety
///
/// `p` came from `arg_mut` with a value of type `T` that lives during the call.
pub unsafe fn mut_arg<'a, T>(p: *mut c_void) -> &'a mut T {
    // SAFETY: the caller keeps the rules of this function.
    unsafe { &mut *p.cast::<T>() }
}

/// Gives the result of the node to the base, in the `Option<T>` of the last argument.
///
/// # Safety
///
/// `p` came from `arg_mut` with an `Option<T>` that lives during the call.
pub unsafe fn put_result<T>(p: *mut c_void, value: T) {
    // SAFETY: the caller keeps the rules of this function.
    unsafe { *p.cast::<Option<T>>() = Some(value) }
}

/// The body of `patina_cluster_slot`: gives the base the slot library (7.1).
///
/// # Safety
///
/// The same as for `ui`. `slot` stays valid until the next call, and no task runs.
pub unsafe fn slot(cluster: *const Cluster, slot: *const SlotRef, rep: *const Reporter) -> bool {
    // SAFETY: the caller keeps the rules of this function.
    let (cluster, rep) = unsafe { (&*cluster, &*rep) };
    catch_unwind(AssertUnwindSafe(|| (cluster.f)(Cmd::Slot(slot), rep))).unwrap_or(false)
}

/// The body of `patina_cluster_call` of a slot library: calls node `node` for a task of the base
/// (7.1). This library has its own thread-locals, so it sets the run and the node again.
///
/// # Safety
///
/// The same as for `task`. `args` points to `len` arguments, as `Slot::call` gives them.
pub unsafe fn call(cluster: *const Cluster, node: u32, args: *const *mut c_void, len: usize, rep: *const Reporter) -> bool {
    // SAFETY: the caller keeps the rules of this function.
    let (cluster, rep, args) = unsafe { (&*cluster, &*rep, std::slice::from_raw_parts(args, len)) };
    let outer = (RUN.replace(rep), NODE.replace(node));
    let ok = catch_unwind(AssertUnwindSafe(|| (cluster.f)(Cmd::Call(node, args), rep))).unwrap_or(false);
    RUN.set(outer.0);
    NODE.set(outer.1);
    ok
}

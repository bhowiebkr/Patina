//! The bodies of the C functions of a cluster (spec 4.7). Each call into node code has its
//! own catch, so a panic never crosses the C ABI.

use std::ffi::c_void;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::Ordering::Relaxed;

use super::{Cluster, Cmd, IDLE, Moved, NODE, RUN, Reporter, catch_unwind};

/// The body of `patina_cluster_new`. Returns null when the parameter defaults panic.
pub fn new_cluster(make: fn() -> Cluster) -> *mut Cluster {
    match catch_unwind(make) {
        Ok(cluster) => Box::into_raw(Box::new(cluster)),
        Err(_) => std::ptr::null_mut(),
    }
}

/// The body of `patina_cluster_set`.
///
/// # Safety
///
/// `cluster` comes from `new_cluster` and is not freed. `text` is null for the default value, or
/// points to `len` bytes. `rep` points to a valid reporter.
pub unsafe fn set(
    cluster: *mut Cluster,
    index: u32,
    text: *const u8,
    len: usize,
    rep: *const Reporter,
) -> bool {
    // SAFETY: the caller keeps the rules of this function.
    let (cluster, rep) = unsafe { (&*cluster, &*rep) };
    let text = match text.is_null() {
        true => None,
        // SAFETY: the caller keeps the rules of this function.
        false => match std::str::from_utf8(unsafe { std::slice::from_raw_parts(text, len) }) {
            Ok(text) => Some(text),
            Err(_) => return false,
        },
    };
    catch_unwind(AssertUnwindSafe(|| (cluster.f)(Cmd::Set(index, text), rep))).unwrap_or(false)
}

/// The body of `patina_cluster_ui`.
///
/// # Safety
///
/// The same as for `set`, and no run of the cluster is active.
pub unsafe fn ui(cluster: *const Cluster, node: u32, mode: u32, text: *const u8, len: usize, rep: *const Reporter) -> bool {
    // SAFETY: the caller keeps the rules of this function.
    let (cluster, rep, bytes) = unsafe { (&*cluster, &*rep, std::slice::from_raw_parts(text, len)) };
    let Ok(events) = std::str::from_utf8(bytes) else {
        return false;
    };
    catch_unwind(AssertUnwindSafe(|| (cluster.f)(Cmd::Ui(node, mode, events), rep))).unwrap_or(false)
}

/// The body of `patina_cluster_parallel`: true when the engine can run the tasks of a run on many
/// threads at a time.
///
/// # Safety
///
/// `cluster` comes from `new_cluster` and is not freed.
pub unsafe fn parallel(cluster: *const Cluster) -> bool {
    // SAFETY: the caller keeps the rules of this function.
    unsafe { (*cluster).parallel }
}

/// The body of `patina_cluster_task`.
///
/// # Safety
///
/// `cluster` comes from `new_cluster`, and is not freed or set during the call. `rep` points to a
/// valid reporter, and its control is valid during the call. Other tasks of the same run can run
/// on other threads at the same time only when `parallel` is true.
pub unsafe fn task(cluster: *const Cluster, task: u32, rep: *const Reporter) -> bool {
    // SAFETY: the caller keeps the rules of this function.
    let (cluster, rep) = unsafe { (&*cluster, &*rep) };
    in_run(rep, || (cluster.f)(Cmd::Task(task), rep))
}

/// The body of `patina_cluster_fire`: one event run of task `task` with the event text (9.4).
///
/// # Safety
///
/// The same as for `task`. `text` points to `len` bytes. Other event runs of the same cluster can
/// run on other threads at the same time only when `parallel` is true.
pub unsafe fn fire(cluster: *const Cluster, task: u32, text: *const u8, len: usize, rep: *const Reporter) -> bool {
    // SAFETY: the caller keeps the rules of this function.
    let (cluster, rep, bytes) = unsafe { (&*cluster, &*rep, std::slice::from_raw_parts(text, len)) };
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    let ok = in_run(rep, || (cluster.f)(Cmd::Fire(task, text), rep));
    rep.set_node(IDLE);
    NODE.set(IDLE);
    if let Some(control) = rep.control() {
        control.lines.store(0, Relaxed);
    }
    let _ = std::io::Write::flush(&mut std::io::stdout());
    ok
}

/// The body of `patina_cluster_begin`, before the first task of a run of domain `domain`.
///
/// # Safety
///
/// The same as for `end`.
pub unsafe fn begin(cluster: *const Cluster, domain: u32, rep: *const Reporter) -> bool {
    // SAFETY: the caller keeps the rules of this function.
    let (cluster, rep) = unsafe { (&*cluster, &*rep) };
    in_run(rep, || (cluster.f)(Cmd::Begin(domain), rep))
}

/// The body of `patina_cluster_end`, after the last task of a run of domain `domain`.
///
/// # Safety
///
/// The same as for `task`, and no task of the cluster runs during the call.
pub unsafe fn end(cluster: *const Cluster, domain: u32, rep: *const Reporter) -> bool {
    // SAFETY: the caller keeps the rules of this function.
    let (cluster, rep) = unsafe { (&*cluster, &*rep) };
    let ok = in_run(rep, || (cluster.f)(Cmd::End(domain), rep));
    rep.set_node(IDLE);
    NODE.set(IDLE);
    if let Some(control) = rep.control() {
        control.lines.store(0, Relaxed);
    }
    // The cluster has its own standard library, which does not flush its stdout at exit.
    let _ = std::io::Write::flush(&mut std::io::stdout());
    ok
}

/// The body of `patina_cluster_take`: the state of node `node` as a box, or null (7.2).
///
/// # Safety
///
/// The same as for `ui`.
pub unsafe fn take(cluster: *const Cluster, node: u32, rep: *const Reporter) -> *mut c_void {
    // SAFETY: the caller keeps the rules of this function.
    let (cluster, rep) = unsafe { (&*cluster, &*rep) };
    let mut out = std::ptr::null_mut();
    let _ = catch_unwind(AssertUnwindSafe(|| (cluster.f)(Cmd::Take(node, &mut out), rep)));
    out
}

/// The body of `patina_cluster_put`: gives `value` to node `node` (7.2). Returns false when the
/// cluster did not take it. Then the value leaks.
///
/// # Safety
///
/// The same as for `ui`. `value` comes from `take` of a cluster, and is not given again. Its state
/// type is the same type as the state type of node `node`.
pub unsafe fn put(cluster: *const Cluster, node: u32, value: *mut c_void, rep: *const Reporter) -> bool {
    // SAFETY: the caller keeps the rules of this function.
    let (cluster, rep) = unsafe { (&*cluster, &*rep) };
    catch_unwind(AssertUnwindSafe(|| (cluster.f)(Cmd::Put(node, Moved(value)), rep))).unwrap_or(false)
}

/// The body of `patina_cluster_export`: a new reference to the value of link `index` (8.2), or
/// null when this cluster does not make it.
///
/// # Safety
///
/// The same as for `ui`.
pub unsafe fn export(cluster: *const Cluster, index: u32, rep: *const Reporter) -> *mut c_void {
    // SAFETY: the caller keeps the rules of this function.
    let (cluster, rep) = unsafe { (&*cluster, &*rep) };
    let mut out = std::ptr::null_mut();
    let _ = catch_unwind(AssertUnwindSafe(|| (cluster.f)(Cmd::Export(index, &mut out), rep)));
    out
}

/// The body of `patina_cluster_import`: links link `index` to `value` (8.2). Returns false when
/// the cluster did not take it. Then the reference leaks.
///
/// # Safety
///
/// The same as for `ui`. `value` comes from `export` of link `index` of a cluster of the same
/// build, and is not given again.
pub unsafe fn import(cluster: *const Cluster, index: u32, value: *mut c_void, rep: *const Reporter) -> bool {
    // SAFETY: the caller keeps the rules of this function.
    let (cluster, rep) = unsafe { (&*cluster, &*rep) };
    catch_unwind(AssertUnwindSafe(|| (cluster.f)(Cmd::Import(index, Moved(value)), rep))).unwrap_or(false)
}

/// Calls `f` with `rep` as the reporter of the task on this thread.
fn in_run(rep: &Reporter, f: impl FnOnce() -> bool) -> bool {
    let outer = RUN.replace(rep);
    NODE.set(IDLE);
    let ok = catch_unwind(AssertUnwindSafe(f)).unwrap_or(false);
    RUN.set(outer);
    NODE.set(IDLE);
    ok
}

/// The body of `patina_cluster_free`. If a drop panics, the rest of the cluster leaks (4.7).
///
/// # Safety
///
/// `cluster` is null, or comes from `new_cluster` and is not freed.
pub unsafe fn free(cluster: *mut Cluster) {
    if !cluster.is_null() {
        // SAFETY: the caller gives a cluster from `new_cluster`, and frees it one time.
        let cluster = unsafe { Box::from_raw(cluster) };
        let _ = catch_unwind(AssertUnwindSafe(move || drop(cluster)));
    }
}

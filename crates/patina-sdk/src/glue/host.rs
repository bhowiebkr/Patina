//! The share function and the project root that the engine gives to a cluster (spec 10.2).

use std::ffi::c_void;
use std::sync::{Mutex, OnceLock, PoisonError};

/// The engine function that keeps one value for each key in the process (10.2). When the key has
/// no value and `value` is not null, it keeps `value`. It gives the value of the key, or null.
pub type ShareFn = unsafe extern "C" fn(key: *const u8, len: usize, value: *mut c_void) -> *mut c_void;

/// The share function of the engine and the project root.
static HOST: OnceLock<(ShareFn, String)> = OnceLock::new();

/// The body of `patina_cluster_host`. The engine calls it before `patina_cluster_new`.
///
/// # Safety
///
/// `root` points to `len` bytes.
pub unsafe fn set_host(share: ShareFn, root: *const u8, len: usize) {
    // SAFETY: the caller gives `len` bytes at `root`.
    let root = unsafe { std::slice::from_raw_parts(root, len) };
    let _ = HOST.set((share, String::from_utf8_lossy(root).into_owned()));
}

/// The body of `patina_sdk::asset!`. With no engine, as in a test, the file is in the crate folder.
pub fn asset(crate_dir: &str, file: &str) -> std::path::PathBuf {
    asset_in(HOST.get().map(|(_, root)| root.as_str()), crate_dir, file)
}

pub(super) fn asset_in(root: Option<&str>, crate_dir: &str, file: &str) -> std::path::PathBuf {
    let dir = std::path::Path::new(crate_dir);
    let Some((root, mut parts)) = root.map(|root| (root, dir.iter().rev())) else {
        return dir.join("assets").join(file);
    };
    // A node crate is in `<group>/<name>` of the project, for example `nodes/blur` (6.2).
    let (name, group) = (parts.next().unwrap_or_default(), parts.next().unwrap_or_default());
    std::path::Path::new(root).join(group).join(name).join("assets").join(file)
}

/// The body of `patina_sdk::project_path`. An absolute path stays the same.
pub fn project_path(path: &str) -> std::path::PathBuf {
    match HOST.get() {
        Some((_, root)) => std::path::Path::new(root).join(path),
        None => path.into(),
    }
}

/// The `ShareFn` of the engine. A cluster with no engine, as in a test, uses its own copy.
///
/// # Safety
///
/// `key` points to `len` bytes. `value` is null or a value that stays until the process ends.
pub unsafe extern "C" fn share(key: *const u8, len: usize, value: *mut c_void) -> *mut c_void {
    static VALUES: Mutex<Vec<(Box<[u8]>, usize)>> = Mutex::new(Vec::new());
    // SAFETY: the caller gives `len` bytes at `key`.
    let key = unsafe { std::slice::from_raw_parts(key, len) };
    let mut values = VALUES.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(&(_, kept)) = values.iter().find(|(k, _)| **k == *key) {
        return kept as *mut c_void;
    }
    if !value.is_null() {
        values.push((key.into(), value as usize));
    }
    value
}

/// The body of `patina_sdk::shared`. The clusters and the slot of one project have the same
/// toolchain and the same source, so a type has the same layout in each (7.2). The key holds the
/// project root, so the projects of one editor process do not share.
pub fn shared<T: Send + Sync + 'static>(make: impl FnOnce() -> T) -> &'static T {
    let (share, root) = HOST.get().map_or((share as ShareFn, ""), |(f, root)| (*f, root.as_str()));
    shared_with(share, root, make)
}

/// The shared value of the project `root` for the engine, which gives `share` to the clusters.
pub fn shared_in<T: Send + Sync + 'static>(root: &str, make: impl FnOnce() -> T) -> &'static T {
    shared_with(share, root, make)
}

fn shared_with<T: Send + Sync + 'static>(share: ShareFn, root: &str, make: impl FnOnce() -> T) -> &'static T {
    let key = format!("{root} {} {} {}", std::any::type_name::<T>(), size_of::<T>(), align_of::<T>());
    // SAFETY: the key is valid during the calls, and a kept value is never freed.
    let value = unsafe {
        let mut value = share(key.as_ptr(), key.len(), std::ptr::null_mut());
        if value.is_null() {
            let mine = Box::into_raw(Box::new(make())).cast::<c_void>();
            value = share(key.as_ptr(), key.len(), mine);
            if value != mine {
                // Another thread kept its value first.
                drop(Box::from_raw(mine.cast::<T>()));
            }
        }
        value
    };
    // SAFETY: the key holds the `TypeId` of `T`, so the value of the key is a `T`.
    unsafe { &*value.cast::<T>() }
}

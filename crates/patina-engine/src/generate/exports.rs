//! The exported functions of each generated cluster crate (spec 8.2), and its logger (15.7).

pub(super) const EXPORTS: &str = r#"
#[unsafe(no_mangle)]
pub extern "C" fn patina_abi_version() -> u32 {
    ::patina_sdk::glue::ABI_VERSION
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn patina_cluster_host(share: ::patina_sdk::glue::ShareFn, root: *const u8, len: usize) {
    // SAFETY: the engine gives `len` bytes at `root`.
    unsafe { ::patina_sdk::glue::set_host(share, root, len) }
}

#[unsafe(no_mangle)]
pub extern "C" fn patina_cluster_new() -> *mut ::std::ffi::c_void {
    ::patina_sdk::glue::new_cluster(make).cast()
}

/// # Safety
///
/// The engine keeps the rules of `patina_sdk::glue::set`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn patina_cluster_set(
    cluster: *mut ::std::ffi::c_void,
    index: u32,
    text: *const u8,
    len: usize,
    rep: *const ::patina_sdk::glue::Reporter,
) -> bool {
    // SAFETY: the engine keeps the rules of `glue::set`.
    unsafe { ::patina_sdk::glue::set(cluster.cast(), index, text, len, rep) }
}

/// # Safety
///
/// The engine keeps the rules of `patina_sdk::glue::ui`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn patina_cluster_ui(
    cluster: *const ::std::ffi::c_void,
    node: u32,
    mode: u32,
    text: *const u8,
    len: usize,
    rep: *const ::patina_sdk::glue::Reporter,
) -> bool {
    // SAFETY: the engine keeps the rules of `glue::ui`.
    unsafe { ::patina_sdk::glue::ui(cluster.cast(), node, mode, text, len, rep) }
}

/// # Safety
///
/// The engine keeps the rules of `patina_sdk::glue::parallel`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn patina_cluster_parallel(cluster: *const ::std::ffi::c_void) -> bool {
    // SAFETY: the engine keeps the rules of `glue::parallel`.
    unsafe { ::patina_sdk::glue::parallel(cluster.cast()) }
}

/// # Safety
///
/// The engine keeps the rules of `patina_sdk::glue::task`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn patina_cluster_task(
    cluster: *const ::std::ffi::c_void,
    task: u32,
    rep: *const ::patina_sdk::glue::Reporter,
) -> bool {
    // SAFETY: the engine keeps the rules of `glue::task`.
    unsafe { ::patina_sdk::glue::task(cluster.cast(), task, rep) }
}

/// # Safety
///
/// The engine keeps the rules of `patina_sdk::glue::begin`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn patina_cluster_begin(
    cluster: *const ::std::ffi::c_void,
    domain: u32,
    rep: *const ::patina_sdk::glue::Reporter,
) -> bool {
    // SAFETY: the engine keeps the rules of `glue::begin`.
    unsafe { ::patina_sdk::glue::begin(cluster.cast(), domain, rep) }
}

/// # Safety
///
/// The engine keeps the rules of `patina_sdk::glue::end`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn patina_cluster_end(
    cluster: *const ::std::ffi::c_void,
    domain: u32,
    rep: *const ::patina_sdk::glue::Reporter,
) -> bool {
    // SAFETY: the engine keeps the rules of `glue::end`.
    unsafe { ::patina_sdk::glue::end(cluster.cast(), domain, rep) }
}

/// # Safety
///
/// The engine keeps the rules of `patina_sdk::glue::take`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn patina_cluster_take(
    cluster: *const ::std::ffi::c_void,
    node: u32,
    rep: *const ::patina_sdk::glue::Reporter,
) -> *mut ::std::ffi::c_void {
    // SAFETY: the engine keeps the rules of `glue::take`.
    unsafe { ::patina_sdk::glue::take(cluster.cast(), node, rep) }
}

/// # Safety
///
/// The engine keeps the rules of `patina_sdk::glue::read`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn patina_cluster_read(
    cluster: *const ::std::ffi::c_void,
    index: u32,
    rep: *const ::patina_sdk::glue::Reporter,
) -> *mut ::std::ffi::c_void {
    // SAFETY: the engine keeps the rules of `glue::read`.
    unsafe { ::patina_sdk::glue::read(cluster.cast(), index, rep) }
}

/// # Safety
///
/// The engine keeps the rules of `patina_sdk::glue::put`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn patina_cluster_put(
    cluster: *const ::std::ffi::c_void,
    node: u32,
    value: *mut ::std::ffi::c_void,
    rep: *const ::patina_sdk::glue::Reporter,
) -> bool {
    // SAFETY: the engine keeps the rules of `glue::put`.
    unsafe { ::patina_sdk::glue::put(cluster.cast(), node, value, rep) }
}

/// # Safety
///
/// The engine keeps the rules of `patina_sdk::glue::export`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn patina_cluster_export(
    cluster: *const ::std::ffi::c_void,
    index: u32,
    rep: *const ::patina_sdk::glue::Reporter,
) -> *mut ::std::ffi::c_void {
    // SAFETY: the engine keeps the rules of `glue::export`.
    unsafe { ::patina_sdk::glue::export(cluster.cast(), index, rep) }
}

/// # Safety
///
/// The engine keeps the rules of `patina_sdk::glue::import`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn patina_cluster_import(
    cluster: *const ::std::ffi::c_void,
    index: u32,
    value: *mut ::std::ffi::c_void,
    rep: *const ::patina_sdk::glue::Reporter,
) -> bool {
    // SAFETY: the engine keeps the rules of `glue::import`.
    unsafe { ::patina_sdk::glue::import(cluster.cast(), index, value, rep) }
}

/// # Safety
///
/// The engine keeps the rules of `patina_sdk::glue::slot`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn patina_cluster_slot(
    cluster: *const ::std::ffi::c_void,
    slot: *const ::patina_sdk::glue::SlotRef,
    rep: *const ::patina_sdk::glue::Reporter,
) -> bool {
    // SAFETY: the engine keeps the rules of `glue::slot`.
    unsafe { ::patina_sdk::glue::slot(cluster.cast(), slot, rep) }
}

/// # Safety
///
/// The base keeps the rules of `patina_sdk::glue::call`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn patina_cluster_call(
    cluster: *const ::std::ffi::c_void,
    node: u32,
    args: *const *mut ::std::ffi::c_void,
    len: usize,
    rep: *const ::patina_sdk::glue::Reporter,
) -> bool {
    // SAFETY: the base keeps the rules of `glue::call`.
    unsafe { ::patina_sdk::glue::call(cluster.cast(), node, args, len, rep) }
}

/// # Safety
///
/// The engine keeps the rules of `patina_sdk::glue::free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn patina_cluster_free(cluster: *mut ::std::ffi::c_void) {
    // SAFETY: the engine keeps the rules of `glue::free`.
    unsafe { ::patina_sdk::glue::free(cluster.cast()) }
}
"#;

/// Sends the `log` records of the nodes to the engine (15.7).
pub(super) const LOGGER: &str = r#"
struct PatinaLog;

impl ::patina_log::Log for PatinaLog {
    fn enabled(&self, metadata: &::patina_log::Metadata<'_>) -> bool {
        metadata.level() <= ::patina_log::Level::Info
    }

    fn log(&self, record: &::patina_log::Record<'_>) {
        if self.enabled(record.metadata()) {
            ::patina_sdk::glue::log_line(record.level().as_str(), record.args());
        }
    }

    fn flush(&self) {}
}
"#;

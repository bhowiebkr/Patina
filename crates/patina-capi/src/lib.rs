//! The C API of a Patina package (spec 14.2). `patina_capi.h` declares the functions.

use std::cell::RefCell;
use std::ffi::{CStr, CString, c_char, c_int};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

use patina_engine::{Checked, Cluster, Control, Outcome, package};

/// An open package.
pub struct PatinaPackage {
    checked: Checked,
    cluster: Cluster,
    control: Control,
    /// The outputs of the last run or frame, as "<node ID>.<output key>" and the text. The
    /// strings stay, so a frame does not allocate when the texts keep their size.
    values: Vec<(String, String)>,
    /// The text that `patina_value` gave last, with its zero byte.
    text: Vec<u8>,
    mono: [f32; 256],
}

thread_local! {
    static ERROR: RefCell<CString> = RefCell::new(CString::default());
}

fn set_error(text: String) {
    let text = CString::new(text.replace('\0', " ")).expect("the zero bytes are replaced");
    ERROR.with(|e| *e.borrow_mut() = text);
}

/// Runs `f`, and turns an error or a panic into `fail` and the text of `patina_error`.
fn guard<T>(fail: T, f: impl FnOnce() -> Result<T, String>) -> T {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(value)) => value,
        Ok(Err(text)) => {
            set_error(text);
            fail
        }
        Err(_) => {
            set_error("a panic in Patina".to_owned());
            fail
        }
    }
}

/// # Safety
/// `text` is null or a C string that stays valid during the call.
unsafe fn text<'a>(text: *const c_char, what: &str) -> Result<&'a str, String> {
    if text.is_null() {
        return Err(format!("the {what} is null"));
    }
    // SAFETY: the caller gives a valid C string, as the header says.
    unsafe { CStr::from_ptr(text) }.to_str().map_err(|_| format!("the {what} is not UTF-8"))
}

/// # Safety
/// `package` is null or a pointer from `patina_open` that is not closed.
unsafe fn open_package<'a>(package: *mut PatinaPackage) -> Result<&'a mut PatinaPackage, String> {
    // SAFETY: the caller gives a pointer from `patina_open`, and one thread uses it at a time.
    unsafe { package.as_mut() }.ok_or_else(|| "the package is null".to_owned())
}

/// Keeps the values and the error of an outcome. Gives 0, or -1 when a node failed.
fn keep(values: &mut Vec<(String, String)>, outcome: &Outcome) -> Result<c_int, String> {
    values.truncate(outcome.values.len());
    for (i, value) in outcome.values.iter().enumerate() {
        match values.get_mut(i) {
            Some((from, text)) => {
                from.clone_from(&value.from);
                text.clone_from(&value.text);
            }
            None => values.push((value.from.clone(), value.text.clone())),
        }
    }
    if outcome.ok {
        return Ok(0);
    }
    let reports: Vec<String> = outcome.reports.iter().map(ToString::to_string).collect();
    let skipped = outcome.skipped.iter().map(|id| format!("Node {id}: skipped: an input has no value"));
    Err(reports.into_iter().chain(skipped).collect::<Vec<_>>().join("\n"))
}

/// # Safety
/// `dir` is a C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn patina_open(dir: *const c_char) -> *mut PatinaPackage {
    guard(std::ptr::null_mut(), || {
        // SAFETY: the caller keeps the contract of this function.
        let dir = unsafe { text(dir, "folder") }?;
        let (checked, cluster) = package::open(Path::new(dir)).map_err(|e| e.to_string())?;
        let package = PatinaPackage { checked, cluster, control: Control::default(), values: Vec::new(), text: Vec::new(), mono: [0.0; 256] };
        Ok(Box::into_raw(Box::new(package)))
    })
}

/// # Safety
/// `package` comes from `patina_open`. `name` and `value` are C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn patina_set(package: *mut PatinaPackage, name: *const c_char, value: *const c_char) -> c_int {
    guard(-1, || {
        // SAFETY: the caller keeps the contract of this function.
        let (package, name, value) = unsafe { (open_package(package)?, text(name, "name")?, text(value, "value")?) };
        package::set_args(&mut package.checked, &[format!("{name}={value}")])?;
        Ok(0)
    })
}

/// # Safety
/// `package` comes from `patina_open`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn patina_run(package: *mut PatinaPackage) -> c_int {
    guard(-1, || {
        // SAFETY: the caller keeps the contract of this function.
        let package = unsafe { open_package(package) }?;
        // A node with a bad parameter does not run, so the graph does not run.
        keep(&mut package.values, &package.cluster.set_params(package.checked.graph()))?;
        keep(&mut package.values, &package.cluster.run(&package.control))
    })
}

/// # Safety
/// `package` comes from `patina_open`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn patina_frame(package: *mut PatinaPackage) -> c_int {
    guard(-1, || {
        // SAFETY: the caller keeps the contract of this function.
        let package = unsafe { open_package(package) }?;
        if !package.cluster.frames() {
            return Err("the graph has no frame nodes".to_owned());
        }
        keep(&mut package.values, package.cluster.frame(&package.control))
    })
}

/// # Safety
/// `package` comes from `patina_open`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn patina_block(package: *mut PatinaPackage) -> c_int {
    guard(-1, || {
        // SAFETY: the caller keeps the contract of this function.
        let package = unsafe { open_package(package) }?;
        if !package.cluster.blocks() {
            return Err("the graph has no audio nodes".to_owned());
        }
        keep(&mut package.values, package.cluster.block(&package.control))
    })
}

/// # Safety
/// `package` comes from `patina_open`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn patina_sound(package: *mut PatinaPackage) -> *const f32 {
    guard(std::ptr::null(), || {
        // SAFETY: the caller keeps the contract of this function.
        let package = unsafe { open_package(package) }?;
        let Some(sound) = package.cluster.sound() else { return Ok(std::ptr::null()) };
        for (mono, pair) in package.mono.iter_mut().zip(sound.as_chunks::<2>().0) {
            *mono = (pair[0] + pair[1]) * 0.5;
        }
        Ok(package.mono.as_ptr())
    })
}

/// # Safety
/// `package` comes from `patina_open`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn patina_stereo_sound(package: *mut PatinaPackage) -> *const f32 {
    guard(std::ptr::null(), || {
        // SAFETY: the caller keeps the contract of this function.
        let package = unsafe { open_package(package) }?;
        Ok(package.cluster.sound().map_or(std::ptr::null(), |sound| sound.as_ptr()))
    })
}

/// # Safety
/// `package` comes from `patina_open`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn patina_events(package: *mut PatinaPackage, count: u64) -> c_int {
    guard(-1, || {
        // SAFETY: the caller keeps the contract of this function.
        let package = unsafe { open_package(package) }?;
        if !package.cluster.events() {
            return Err("the graph has no event nodes".to_owned());
        }
        // The engine reads the settings of the triggers (9.4).
        keep(&mut package.values, &package.cluster.set_params(package.checked.graph()))?;
        let (values, mut failed) = (&mut package.values, None);
        package.cluster.run_events(count, &package.control, |run| {
            if let Err(text) = keep(values, &run.out) {
                failed.get_or_insert(format!("Run {}: {text}", run.id));
            }
        });
        failed.map_or(Ok(0), Err)
    })
}

/// # Safety
/// `package` comes from `patina_open`. `name` is a C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn patina_value(package: *mut PatinaPackage, name: *const c_char) -> *const c_char {
    guard(std::ptr::null(), || {
        // SAFETY: the caller keeps the contract of this function.
        let (package, name) = unsafe { (open_package(package)?, text(name, "name")?) };
        let Some((_, value)) = package.values.iter().find(|(from, _)| from == name) else { return Ok(std::ptr::null()) };
        package.text.clear();
        package.text.extend(value.bytes().filter(|&b| b != 0));
        package.text.push(0);
        Ok(package.text.as_ptr().cast())
    })
}

/// The text stays until the next error on this thread.
#[unsafe(no_mangle)]
pub extern "C" fn patina_error() -> *const c_char {
    ERROR.with(|e| e.borrow().as_ptr())
}

/// # Safety
/// `package` is null or comes from `patina_open`, and nothing uses it after this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn patina_close(package: *mut PatinaPackage) {
    if !package.is_null() {
        // SAFETY: the pointer came from `Box::into_raw` in `patina_open`.
        drop(unsafe { Box::from_raw(package) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn error() -> String {
        // SAFETY: `patina_error` gives a valid C string.
        unsafe { CStr::from_ptr(patina_error()) }.to_string_lossy().into_owned()
    }

    #[test]
    fn errors_give_null_or_minus_one_and_a_text() {
        // SAFETY: each pointer is null or a valid C string.
        unsafe {
            assert!(patina_open(std::ptr::null()).is_null());
            assert_eq!(error(), "the folder is null");
            assert!(patina_open(c"no such package".as_ptr()).is_null());
            assert!(!error().is_empty());
            assert_eq!(patina_run(std::ptr::null_mut()), -1);
            assert_eq!(error(), "the package is null");
            assert_eq!(patina_set(std::ptr::null_mut(), c"a.b".as_ptr(), c"1".as_ptr()), -1);
            assert!(patina_value(std::ptr::null_mut(), c"a.b".as_ptr()).is_null());
            assert_eq!(patina_block(std::ptr::null_mut()), -1);
            assert!(patina_sound(std::ptr::null_mut()).is_null());
            assert!(patina_stereo_sound(std::ptr::null_mut()).is_null());
            assert_eq!(patina_events(std::ptr::null_mut(), 1), -1);
            patina_close(std::ptr::null_mut());
        }
    }
}

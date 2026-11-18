//! Loads the built `patina_capi` library, as a host program does (spec 14.2). `cargo test`
//! links the rlib only, so this test builds the library first.

use std::ffi::{CStr, c_char, c_int, c_void};
use std::path::PathBuf;
use std::process::Command;

use libloading::{Library, Symbol};

/// Builds the library with cargo and gives the path that cargo reports.
fn build() -> PathBuf {
    let out = Command::new(env!("CARGO"))
        .args(["build", "-p", "patina-capi", "--locked", "--message-format=json-render-diagnostics"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo starts");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let name = format!("{}patina_capi{}", std::env::consts::DLL_PREFIX, std::env::consts::DLL_SUFFIX);
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter_map(|message| message["filenames"].as_array().cloned())
        .flatten()
        .filter_map(|file| file.as_str().map(PathBuf::from))
        .find(|file| file.file_name().is_some_and(|f| f == name.as_str()))
        .expect("cargo reports the library")
}

/// The function names that `patina_capi.h` declares.
fn header_functions() -> Vec<&'static str> {
    let names: Vec<_> = include_str!("../patina_capi.h")
        .lines()
        .filter_map(|line| line.split_once('(').and_then(|(head, _)| head.rsplit([' ', '*']).next()))
        .filter(|name| name.starts_with("patina_"))
        .collect();
    assert!(names.len() >= 10, "the header gives {names:?}");
    names
}

#[test]
fn the_built_library_has_each_function_of_the_header() {
    let path = build();
    // SAFETY: the library is this crate, and loading it runs no code of its own.
    let lib = unsafe { Library::new(&path) }.expect("the library loads");
    // SAFETY: the types are the ones of `src/lib.rs`, and each pointer is null.
    unsafe {
        for name in header_functions() {
            assert!(lib.get::<*const c_void>(name.as_bytes()).is_ok(), "{name} is not exported");
        }
        let open: Symbol<unsafe extern "C" fn(*const c_char) -> *mut c_void> = lib.get(b"patina_open").unwrap();
        let run: Symbol<unsafe extern "C" fn(*mut c_void) -> c_int> = lib.get(b"patina_run").unwrap();
        let error: Symbol<extern "C" fn() -> *const c_char> = lib.get(b"patina_error").unwrap();
        let close: Symbol<unsafe extern "C" fn(*mut c_void)> = lib.get(b"patina_close").unwrap();
        assert!(open(std::ptr::null()).is_null());
        assert_eq!(CStr::from_ptr(error()).to_str(), Ok("the folder is null"));
        assert_eq!(run(std::ptr::null_mut()), -1);
        assert_eq!(CStr::from_ptr(error()).to_str(), Ok("the package is null"));
        close(std::ptr::null_mut());
    }
}

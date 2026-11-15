//! A local crate with C code, built with cc-rs.

unsafe extern "C" {
    fn mix_numbers(a: i32, b: i32) -> i32;
}

pub fn mix(a: i32, b: i32) -> i32 {
    // SAFETY: mix_numbers reads only its two arguments.
    unsafe { mix_numbers(a, b) }
}

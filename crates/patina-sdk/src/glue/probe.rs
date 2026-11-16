//! The probes of the held values (15.9): their text and their lanes (6.3). The generated code
//! has no closure for a probe, so a large cluster builds fast.

use std::fmt::{self, Write as _};
use std::panic::AssertUnwindSafe;

use super::{LANES, Reporter, TEXT_LEN, VALUE, catch_unwind};
use crate::{Lanes, Text, Viewer};

/// A held value for its text. The generated code calls `(&&&Probe(v)).text_fn()`, so Rust uses
/// `ViewerText` when the type implements `Viewer`, then `DebugText` for `Debug`, and `NameText`
/// if not (5.3).
pub struct Probe<'a, T>(pub &'a T);

/// The function that writes the text of a value of type `T`.
pub type TextFn<T> = fn(&T, &mut Text<'_>);

pub trait ViewerText<T> {
    fn text_fn(&self) -> TextFn<T>;
}

impl<T: Viewer> ViewerText<T> for &&Probe<'_, T> {
    fn text_fn(&self) -> TextFn<T> {
        |value, out| {
            let _ = Viewer::text(value, out);
        }
    }
}

pub trait DebugText<T> {
    fn text_fn(&self) -> TextFn<T>;
}

impl<T: fmt::Debug> DebugText<T> for &Probe<'_, T> {
    fn text_fn(&self) -> TextFn<T> {
        |value, out| {
            let _ = write!(out, "{value:?}");
        }
    }
}

pub trait NameText<T> {
    fn text_fn(&self) -> TextFn<T>;
}

impl<T> NameText<T> for Probe<'_, T> {
    fn text_fn(&self) -> TextFn<T> {
        |_, out| {
            let _ = write!(out, "{} ({} bytes)", std::any::type_name::<T>(), size_of::<T>());
        }
    }
}

/// Sends the text of held value `index` (15.9), with its own catch.
// shortcut: each run formats and sends each held value, send them at the display rate (15.6) when a graph runs faster than the display.
pub fn send_value<T>(rep: &Reporter, index: u32, value: &T, text: TextFn<T>) {
    let mut buf = [0; TEXT_LEN + 3];
    let mut out = Text::new(&mut buf);
    if catch_unwind(AssertUnwindSafe(|| text(value, &mut out))).is_err() {
        out.clear();
        let _ = out.write_str("the text of the value panicked");
    }
    rep.send(index, VALUE, out.finish());
}

/// Sends the lanes of held value `index` (6.3), with its own catch. A panic sends no lanes.
pub fn send_lanes<T>(rep: &Reporter, index: u32, value: &T, lanes: impl FnOnce(&T) -> Lanes) {
    let Ok(lanes) = catch_unwind(AssertUnwindSafe(|| lanes(value))) else { return };
    let mut buf = [0; 64 + 1 + 64 + 3];
    let mut out = Text::new(&mut buf);
    for i in (0..lanes.count).rev() {
        let _ = out.write_char(if lanes.bits >> i & 1 == 1 { '1' } else { '0' });
    }
    let _ = write!(out, " {}", lanes.label);
    rep.send(index, LANES, out.finish());
}

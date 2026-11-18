//! The probes of the held values (15.9): their text, their thumbnail and widgets (5.3) and their
//! lanes (6.3). The generated code has no closure for a probe, so a large cluster builds fast.

use std::cell::RefCell;
use std::fmt::{self, Write as _};
use std::panic::AssertUnwindSafe;

use super::{LANES, Reporter, TEXT_LEN, THUMB, VALUE, VIEW, WAVE, WAVE_LEN, catch_unwind};
use crate::{Lanes, Text, Thumbnail, Ui, Viewer};

/// A held value for its text. The generated code calls `(&&&Probe(v)).view_fns()`, so Rust uses
/// `ViewerText` when the type implements `Viewer`, then `DebugText` for `Debug`, and `NameText`
/// if not (5.3).
pub struct Probe<'a, T>(pub &'a T);

/// The function that writes the text of a value of type `T`.
pub type TextFn<T> = fn(&T, &mut Text<'_>);
/// The function that gives a value of type `T` as a `Viewer`, for its thumbnail and its widgets.
pub type ViewerFn<T> = fn(&T) -> &dyn Viewer;
/// The text function and, for a `Viewer`, the viewer function of a value.
pub type ViewFns<T> = (TextFn<T>, Option<ViewerFn<T>>);

pub trait ViewerText<T> {
    fn view_fns(&self) -> ViewFns<T>;
}

impl<T: Viewer> ViewerText<T> for &&Probe<'_, T> {
    fn view_fns(&self) -> ViewFns<T> {
        let text: TextFn<T> = |value, out| {
            let _ = Viewer::text(value, out);
        };
        let viewer: ViewerFn<T> = |value| value;
        (text, Some(viewer))
    }
}

pub trait DebugText<T> {
    fn view_fns(&self) -> ViewFns<T>;
}

impl<T: fmt::Debug> DebugText<T> for &Probe<'_, T> {
    fn view_fns(&self) -> ViewFns<T> {
        let text: TextFn<T> = |value, out| {
            let _ = write!(out, "{value:?}");
        };
        (text, None)
    }
}

pub trait NameText<T> {
    fn view_fns(&self) -> ViewFns<T>;
}

impl<T> NameText<T> for Probe<'_, T> {
    fn view_fns(&self) -> ViewFns<T> {
        let text: TextFn<T> = |_, out| {
            let _ = write!(out, "{} ({} bytes)", std::any::type_name::<T>(), size_of::<T>());
        };
        (text, None)
    }
}

/// Sends the text, the thumbnail and the widgets of held value `index` (15.9), each with its own
/// catch.
#[inline(never)]
pub fn send_value<T>(rep: &Reporter, index: u32, value: &T, (text, viewer): ViewFns<T>) {
    if rep.quiet() {
        return;
    }
    let mut buf = [0; TEXT_LEN + 3];
    let mut out = Text::new(&mut buf);
    if catch_unwind(AssertUnwindSafe(|| text(value, &mut out))).is_err() {
        out.clear();
        let _ = out.write_str("the text of the value panicked");
    }
    rep.send(index, VALUE, out.finish());
    if let Some(viewer) = viewer {
        send_views(rep, index, viewer(value));
    }
}

/// Sends the thumbnail and the widget lines of held value `index` (5.3). A panic or an empty
/// result sends nothing. The buffers of the thread stay for the next values, so only the first
/// value allocates.
fn send_views(rep: &Reporter, index: u32, viewer: &dyn Viewer) {
    thread_local! {
        static BUFS: RefCell<(Vec<u8>, String)> = const { RefCell::new((Vec::new(), String::new())) };
    }
    BUFS.with(|bufs| {
        // A viewer that sends a value of its own finds the buffers in use, and sends nothing.
        let Ok(mut bufs) = bufs.try_borrow_mut() else { return };
        let (pixels, text) = &mut *bufs;
        let mut out = Thumbnail::new(pixels);
        let drawn = catch_unwind(AssertUnwindSafe(|| viewer.thumbnail(&mut out))).is_ok();
        let [width, height] = out.size();
        if drawn && width > 0 && height > 0 {
            text.clear();
            let _ = write!(text, "{width} {height} ");
            crate::view::push_base64(text, out.data());
            rep.send(index, THUMB, text);
        }
        let mut ui = Ui::viewer(std::mem::take(text));
        let shown = catch_unwind(AssertUnwindSafe(|| viewer.show(&mut ui))).is_ok();
        if shown && !ui.tree().is_empty() {
            rep.send(index, VIEW, ui.tree());
        }
        *text = ui.into_tree();
        let wave = catch_unwind(AssertUnwindSafe(|| viewer.wave()));
        if wave.is_ok_and(|samples| wave_text(samples, text)) {
            rep.send(index, WAVE, text);
        }
    });
}

/// Writes the wave of `samples` (15.9) into `text`: at most `WAVE_LEN` parts, and for each part
/// its sample with the largest size, as an integer from -100 to 100. False when it has no samples.
pub(super) fn wave_text(samples: &[f32], text: &mut String) -> bool {
    text.clear();
    let n = samples.len().min(WAVE_LEN);
    for i in 0..n {
        let part = &samples[i * samples.len() / n..(i + 1) * samples.len() / n];
        let peak = part.iter().copied().fold(0.0f32, |a, s| if s.abs() > a.abs() { s } else { a });
        let space = if i > 0 { " " } else { "" };
        // The fold skips a NaN.
        let _ = write!(text, "{space}{}", (peak.clamp(-1.0, 1.0) * 100.0).round() as i32);
    }
    n > 0
}

/// Sends the lanes of held value `index` (6.3), with its own catch. A panic sends no lanes.
pub fn send_lanes<T>(rep: &Reporter, index: u32, value: &T, lanes: impl FnOnce(&T) -> Lanes) {
    if rep.quiet() {
        return;
    }
    let Ok(lanes) = catch_unwind(AssertUnwindSafe(|| lanes(value))) else { return };
    let mut buf = [0; 64 + 1 + 64 + 3];
    let mut out = Text::new(&mut buf);
    for i in (0..lanes.count).rev() {
        let _ = out.write_char(if lanes.bits >> i & 1 == 1 { '1' } else { '0' });
    }
    let _ = write!(out, " {}", lanes.label);
    rep.send(index, LANES, out.finish());
}

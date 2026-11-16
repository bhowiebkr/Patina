//! The C ABI of a view library (15.8). The `#[view]` attribute writes the exports, and they call
//! these functions.

use std::ffi::c_void;
use std::panic::AssertUnwindSafe;

use super::{EDIT, Reader, Reporter, UI, catch_quiet, catch_unwind, finish};
use crate::{Graph, Ui};

/// The body of `patina_view_new`: a new state from `init` as a raw box, or null when `init`
/// panicked.
pub fn view_new<T>(init: impl FnOnce() -> T) -> *mut c_void {
    catch_unwind(AssertUnwindSafe(init)).map_or(std::ptr::null_mut(), |value| Box::into_raw(Box::new(value)).cast())
}

/// The body of `patina_view_ui`. It calls `ui` with the events and the graph lines (`Graph::new`),
/// then reports each write as `EDIT` and the widget lines as `UI`. False when `ui` panicked. Then
/// the state can be broken, so the engine makes a new state.
///
/// # Safety
///
/// `view` comes from `view_new` of the same `T` and is not freed. `events` and `graph` point to
/// that many bytes, and `rep` to a valid reporter, during the call. `reader` is null or valid
/// during the call, and no run is active.
pub unsafe fn view_ui<T>(
    view: *mut c_void,
    mode: u32,
    events: (*const u8, usize),
    graph: (*const u8, usize),
    rep: *const Reporter,
    reader: *const Reader,
    ui: impl Fn(&mut T, &mut Ui, &mut Graph),
) -> bool {
    // SAFETY: the caller keeps the rules of this function.
    let (view, rep, events, lines, reader) = unsafe {
        (&mut *view.cast::<T>(), &*rep, std::slice::from_raw_parts(events.0, events.1), std::slice::from_raw_parts(graph.0, graph.1), reader.as_ref().copied())
    };
    let (Ok(events), Ok(lines)) = (std::str::from_utf8(events), std::str::from_utf8(lines)) else { return false };
    let mut graph = Graph::with_reader(lines, reader);
    let mut gui = Ui::new(mode, events);
    let caught = catch_quiet(AssertUnwindSafe(|| {
        ui(view, &mut gui, &mut graph);
        // A widget writes its value before it applies its event, so draw again with the new state.
        if !events.is_empty() {
            gui = Ui::new(mode, "");
            ui(view, &mut gui, &mut graph);
        }
        Ok(())
    }));
    let ok = finish(caught, rep, 0);
    if ok {
        graph.edits().for_each(|edit| rep.send(0, EDIT, edit));
        rep.send(0, UI, gui.tree());
    }
    ok
}

/// The body of `patina_view_free`.
///
/// # Safety
///
/// `view` comes from `view_new` of the same `T`, and is not used again. Null is not a state.
pub unsafe fn view_free<T>(view: *mut c_void) {
    if !view.is_null() {
        // SAFETY: the caller keeps the rules of this function.
        let value = unsafe { Box::from_raw(view.cast::<T>()) };
        // A panic in a drop must not cross the C ABI (4.7).
        let _ = catch_unwind(AssertUnwindSafe(move || drop(value)));
    }
}

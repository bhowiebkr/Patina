//! The catch of each call into node code (spec 4.7), and the reports of its errors and panics.

use std::any::Any;
use std::cell::Cell;
use std::panic::{AssertUnwindSafe, UnwindSafe};
use std::sync::Once;

use super::{ERROR, PANIC, Reporter};

/// Calls node code with a catch (4.7). The `dyn` makes one copy of the catch for all nodes, so
/// a graph of many nodes builds fast.
pub fn catch(call: &mut dyn FnMut() -> Result<(), String>) -> std::thread::Result<Result<(), String>> {
    catch_quiet(AssertUnwindSafe(call))
}

/// Reports the result of one node call. An `Err` holds the `Debug` text of the node error.
pub fn finish(caught: std::thread::Result<Result<(), String>>, rep: &Reporter, node: u32) -> bool {
    match caught {
        Ok(Ok(())) => true,
        Ok(Err(message)) => {
            rep.send(node, ERROR, &message);
            false
        }
        Err(payload) => {
            rep.send(node, PANIC, &panic_text(&*payload));
            false
        }
    }
}

/// Reports the result of a call of a stateful node. A panic can break the state, so the state
/// drops, and the next call makes a new state with `init` (spec 4.7).
pub fn finish_state<T>(caught: std::thread::Result<Result<(), String>>, state: &mut Option<T>, rep: &Reporter, node: u32) -> bool {
    let panicked = caught.is_err();
    let ok = finish(caught, rep, node);
    if panicked {
        drop_value(state.take(), rep, node);
    }
    ok
}

/// Drops a value that no node took, with its own catch (spec 4.7).
pub fn drop_value<T>(value: Option<T>, rep: &Reporter, node: u32) -> bool {
    // A type with no drop code cannot panic in a drop, so it needs no catch and no code.
    if !std::mem::needs_drop::<T>() {
        return true;
    }
    rep.set_node(node);
    let caught = catch_quiet(AssertUnwindSafe(move || {
        drop(value);
        Ok(())
    }));
    finish(caught, rep, node)
}

pub(super) fn panic_text(payload: &(dyn Any + Send)) -> String {
    let text = payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("no message");
    match PLACE.take() {
        Some(place) => format!("panic: {text} ({place})"),
        None => format!("panic: {text}"),
    }
}

thread_local! {
    /// True while the innermost catch of this thread reports its panic.
    static QUIET: Cell<bool> = const { Cell::new(false) };
    /// The place in the code of the last caught panic of this thread, for `panic_text`.
    static PLACE: Cell<Option<String>> = const { Cell::new(None) };
}

/// `std::panic::catch_unwind` for a call whose panic goes into a report (4.7). The report has the
/// message and the place, so the standard error does not get the panic message too.
pub fn catch_quiet<R>(f: impl FnOnce() -> R + UnwindSafe) -> std::thread::Result<R> {
    catch_with(true, f)
}

/// `std::panic::catch_unwind` for a call whose panic gives no report, so the standard error gets
/// the panic message. A catch in a quiet catch is loud again.
pub fn catch_unwind<R>(f: impl FnOnce() -> R + UnwindSafe) -> std::thread::Result<R> {
    catch_with(false, f)
}

fn catch_with<R>(quiet: bool, f: impl FnOnce() -> R + UnwindSafe) -> std::thread::Result<R> {
    static HOOK: Once = Once::new();
    if quiet && !std::thread::panicking() {
        HOOK.call_once(quiet_hook);
    }
    let outer = QUIET.replace(quiet);
    let caught = std::panic::catch_unwind(f);
    QUIET.set(outer);
    caught
}

/// Keeps the place of a caught panic, and gives other panics to the hook before it. With
/// `RUST_BACKTRACE`, the hook before it also prints the caught panics with their backtrace.
fn quiet_hook() {
    let backtrace = std::env::var_os("RUST_BACKTRACE").is_some_and(|v| v != "0");
    let before = std::panic::take_hook();
    // The hook stays valid, because a cluster never unloads (8.5).
    std::panic::set_hook(Box::new(move |info| {
        if QUIET.get() {
            PLACE.set(info.location().map(|at| format!("{}:{}:{}", at.file(), at.line(), at.column())));
            if !backtrace {
                return;
            }
        }
        before(info);
    }));
}

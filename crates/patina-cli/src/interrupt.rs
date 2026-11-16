//! Ctrl+C asks the runs to stop, so the CLI ends its loops and the values that the graph keeps
//! drop. A second Ctrl+C ends the process at once.

use std::sync::LazyLock;

use patina_engine::Control;

/// The control of the runs on the main thread. Ctrl+C sets its stop flag.
pub static CONTROL: LazyLock<Control> = LazyLock::new(Control::default);

/// True after Ctrl+C.
pub fn stopped() -> bool {
    CONTROL.stopped()
}

/// Sets the stop flag. Returns false when it was set before.
fn stop() -> bool {
    if stopped() {
        return false;
    }
    CONTROL.set_stop(true);
    true
}

/// Catches Ctrl+C. The first one sets the stop flag, the next one goes to the default handler.
#[cfg(windows)]
pub fn catch() {
    type Handler = extern "system" fn(u32) -> i32;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn SetConsoleCtrlHandler(handler: Option<Handler>, add: i32) -> i32;
    }
    /// Takes `CTRL_C_EVENT` (0) one time. The other events go to the default handler.
    extern "system" fn handler(kind: u32) -> i32 {
        i32::from(kind == 0 && stop())
    }
    LazyLock::force(&CONTROL);
    // SAFETY: the handler is a plain function that lives for the whole process.
    unsafe { SetConsoleCtrlHandler(Some(handler), 1) };
}

/// Catches Ctrl+C. The first one sets the stop flag, the next one ends the process.
#[cfg(unix)]
pub fn catch() {
    type Handler = extern "C" fn(i32);
    const SIGINT: i32 = 2;
    unsafe extern "C" {
        /// `None` is `SIG_DFL`.
        fn signal(signum: i32, handler: Option<Handler>) -> usize;
    }
    /// Reads and stores atomic values and calls `signal`, as a signal handler can.
    extern "C" fn handler(_: i32) {
        stop();
        // SAFETY: `signal` is async-signal-safe.
        unsafe { signal(SIGINT, None) };
    }
    LazyLock::force(&CONTROL);
    // SAFETY: the handler is a plain function that lives for the whole process.
    unsafe { signal(SIGINT, Some(handler)) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_first_ctrl_c_sets_the_stop_flag() {
        assert!(!stopped() && stop() && stopped() && !stop());
    }
}

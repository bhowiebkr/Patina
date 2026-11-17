//! The async runtime of the event nodes (spec 4.6 and 9.4). The engine copies this source into
//! the `build/` folder of each project. A cluster with an async node depends on it, and a node
//! that needs a timer uses `patina_event::tokio`.

use std::fmt;
use std::future::{Future, poll_fn};
use std::panic::AssertUnwindSafe;
use std::sync::OnceLock;
use std::task::Poll;
use std::time::Duration;

use patina_sdk::glue::{self, Args, Caught, Reporter, STOPPED, catch_quiet};
pub use tokio;
use tokio::time::MissedTickBehavior;

/// The longest time between two checks of **Stop** while an async node waits (9.4).
const CHECK: Duration = Duration::from_millis(20);

/// The runtime of the async nodes. The libraries stay loaded (8.5), so its threads stay valid.
/// shortcut: each cluster build makes its own runtime, share one runtime when many builds load.
fn runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("patina-event")
            .enable_all()
            .build()
            .expect("the async runtime starts")
    })
}

/// Calls async node function `f` with `args`, and waits for its future on the thread of the run.
/// None after **Stop**: then the future dropped, and the node reported `STOPPED`.
pub fn run<F, A>(f: F, args: A, rep: &Reporter, node: u32) -> Option<Caught<<A::Out as Future>::Output>>
where
    A: Args<F>,
    A::Out: Future,
{
    match glue::run(f, args) {
        Ok(Ok(future)) => wait(future, rep, node).map(|caught| caught.map(Ok)),
        Ok(Err(message)) => Some(Ok(Err(message))),
        Err(payload) => Some(Err(payload)),
    }
}

/// `run` for a fallible async node. An `Err` gives its `Debug` text.
pub fn try_run<F, T, E, A>(f: F, args: A, rep: &Reporter, node: u32) -> Option<Caught<T>>
where
    E: fmt::Debug,
    A: Args<F>,
    A::Out: Future<Output = Result<T, E>>,
{
    run(f, args, rep, node).map(|caught| caught.map(|value| value.and_then(|r| r.map_err(|e| format!("{e:?}")))))
}

/// Reports the result of `run`, and gives the value of a call that worked.
pub fn done<R>(caught: Option<Caught<R>>, rep: &Reporter, node: u32, ok: &mut bool) -> Option<R> {
    caught.and_then(|caught| glue::done(caught, rep, node, ok))
}

/// Waits for the future of an async node in the slot library (7.1). None after **Stop**, as `run`
/// gives. A panic of the future goes on to the catch of the slot call.
pub fn wait_in_slot<T>(future: impl Future<Output = T>, rep: &Reporter, node: u32) -> Option<T> {
    wait(future, rep, node).map(|caught| caught.unwrap_or_else(|payload| std::panic::resume_unwind(payload)))
}

/// The output type of a future, for the type check of the generated code. It never runs.
pub fn output<F: Future>(_: F) -> F::Output {
    unreachable!("the type check of a cluster never runs")
}

/// Polls `future` on this thread until it is ready or the user clicks **Stop**.
fn wait<T>(future: impl Future<Output = T>, rep: &Reporter, node: u32) -> Option<std::thread::Result<T>> {
    // shortcut: the run thread blocks while it waits. Change it when event runs share the worker threads.
    let mut result = None;
    // The future stays on the stack and drops at the end of the closure, in the catch (4.7).
    let caught = catch_quiet(AssertUnwindSafe(|| {
        let mut future = std::pin::pin!(future);
        result = runtime().block_on(async {
            let mut tick = tokio::time::interval(CHECK);
            tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
            poll_fn(|cx| {
                if patina_sdk::stop_requested() {
                    return Poll::Ready(None);
                }
                match catch_quiet(AssertUnwindSafe(|| future.as_mut().poll(cx))) {
                    Ok(Poll::Ready(value)) => Poll::Ready(Some(Ok(value))),
                    Err(payload) => Poll::Ready(Some(Err(payload))),
                    Ok(Poll::Pending) => {
                        while tick.poll_tick(cx).is_ready() {}
                        Poll::Pending
                    }
                }
            })
            .await
        });
        Ok(())
    }));
    glue::finish(caught, rep, node);
    if result.is_none() {
        rep.send(node, STOPPED, "");
    }
    result
}

#[cfg(test)]
mod tests {
    use std::ffi::c_void;
    use std::future::Future;
    use std::pin::Pin;
    use std::task::{Context, Poll};

    use patina_sdk::glue::{PANIC, Reporter};

    unsafe extern "C" fn keep(ctx: *mut c_void, node: u32, kind: u32, text: *const u8, len: usize) {
        // SAFETY: the test gives a `Vec` as the context, and a text that is valid during the call.
        let (sent, bytes) = unsafe { (&mut *ctx.cast::<Vec<(u32, u32, String)>>(), std::slice::from_raw_parts(text, len)) };
        sent.push((node, kind, String::from_utf8_lossy(bytes).into_owned()));
    }

    /// A future that is ready at once, and panics when it drops.
    struct Loud;

    impl Future for Loud {
        type Output = u8;
        fn poll(self: Pin<&mut Self>, _: &mut Context) -> Poll<u8> {
            Poll::Ready(7)
        }
    }

    impl Drop for Loud {
        fn drop(&mut self) {
            panic!("loud drop");
        }
    }

    #[test]
    fn a_panic_in_the_drop_of_a_future_goes_to_its_node() {
        let mut sent: Vec<(u32, u32, String)> = Vec::new();
        let rep = Reporter { ctx: (&raw mut sent).cast(), report: keep, control: std::ptr::null() };
        assert!(matches!(super::wait(Loud, &rep, 3), Some(Ok(7))));
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].0, 3);
        assert_eq!(sent[0].1, PANIC);
        assert!(sent[0].2.contains("loud drop"), "{}", sent[0].2);
    }
}

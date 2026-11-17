//! The node calls of a fused cluster (spec 4.7). Each call has its own catch, but the generated
//! code has no closure: a closure for each node makes the type check of a large cluster slow.

use std::fmt;
use std::panic::AssertUnwindSafe;

use super::{Reporter, catch_quiet, drop_value, finish};

/// The result of `run`: a panic, an error of a fallible node, or the value of the call.
pub type Caught<R> = std::thread::Result<Result<R, String>>;

/// One argument of a node call. `get` runs in the catch of the node.
pub trait Arg {
    type Value;
    fn get(self) -> Self::Value;
}

impl<'a, T: ?Sized> Arg for &'a T {
    type Value = &'a T;
    fn get(self) -> &'a T {
        self
    }
}

impl<'a, T: ?Sized> Arg for &'a mut T {
    type Value = &'a mut T;
    fn get(self) -> &'a mut T {
        self
    }
}

/// A value that the node takes.
pub struct Take<T>(pub T);

impl<T> Arg for Take<T> {
    type Value = T;
    fn get(self) -> T {
        self.0
    }
}

/// A copy of a value that other nodes also read. A panic in `clone` is a panic of the node.
pub struct Cloned<'a, T>(pub &'a T);

impl<T: Clone> Arg for Cloned<'_, T> {
    type Value = T;
    fn get(self) -> T {
        self.0.clone()
    }
}

/// The state of a stateful node. A missing state comes from `init` (4.2), in the catch.
pub struct Init<'a, T, I>(pub &'a mut Option<T>, pub I);

impl<'a, T, I: FnOnce() -> T> Arg for Init<'a, T, I> {
    type Value = &'a mut T;
    fn get(self) -> &'a mut T {
        self.0.get_or_insert_with(self.1)
    }
}

/// A value that a `&mut` input changes. The value stays in the `Option` after the call.
pub struct Hold<'a, A: Arg>(pub &'a mut Option<A::Value>, pub A);

impl<'a, A: Arg> Arg for Hold<'a, A> {
    type Value = &'a mut A::Value;
    fn get(self) -> &'a mut A::Value {
        self.0.insert(self.1.get())
    }
}

/// A tuple of arguments for node function `F`.
pub trait Args<F> {
    type Out;
    fn call(self, f: F) -> Self::Out;
}

macro_rules! args {
    ($($a:ident)*) => {
        impl<F: FnOnce($($a::Value),*) -> R, R, $($a: Arg),*> Args<F> for ($($a,)*) {
            type Out = R;
            #[allow(non_snake_case, reason = "the names come from the type parameters")]
            fn call(self, f: F) -> R {
                let ($($a,)*) = self;
                f($($a.get()),*)
            }
        }
    };
}

args!();
args!(A);
args!(A B);
args!(A B C);
args!(A B C D);
args!(A B C D E);
args!(A B C D E G);
args!(A B C D E G H);
args!(A B C D E G H J);
args!(A B C D E G H J K);
args!(A B C D E G H J K L);
args!(A B C D E G H J K L M);
args!(A B C D E G H J K L M N);
args!(A B C D E G H J K L M N O);
args!(A B C D E G H J K L M N O P);
args!(A B C D E G H J K L M N O P Q);
args!(A B C D E G H J K L M N O P Q S);

/// Calls node function `f` with `args` in a catch (4.7).
pub fn run<F, A: Args<F>>(f: F, args: A) -> Caught<A::Out> {
    catch_quiet(AssertUnwindSafe(move || Ok(args.call(f))))
}

/// Calls fallible node function `f` with `args` in a catch. An `Err` gives its `Debug` text.
pub fn try_run<F, T, E: fmt::Debug, A: Args<F, Out = Result<T, E>>>(f: F, args: A) -> Caught<T> {
    catch_quiet(AssertUnwindSafe(move || args.call(f).map_err(|e| format!("{e:?}"))))
}

/// Calls plain node function `f` with `args` in a catch, and times the call. A call that worked
/// stores its value in `out`. Each step of the same function and argument types shares this one
/// copy of the code, so a cluster of many gates builds fast. Returns false when the call failed.
#[inline(never)]
pub fn step<F, A: Args<F>>(rep: &Reporter, node: u32, f: F, args: A, out: &mut Option<A::Out>) -> bool {
    let (start, mut ok) = (rep.start(), true);
    if let Some(value) = done(run(f, args), rep, node, &mut ok) {
        *out = Some(value);
    }
    rep.time(node, start);
    ok
}

/// Reports the result of `run`, and gives the value of a call that worked.
pub fn done<R>(caught: Caught<R>, rep: &Reporter, node: u32, ok: &mut bool) -> Option<R> {
    let caught = match caught {
        Ok(Ok(value)) => return Some(value),
        Ok(Err(message)) => Ok(Err(message)),
        Err(payload) => Err(payload),
    };
    *ok &= finish(caught, rep, node);
    None
}

/// `done` for a stateful node. A panic can break the state, so the state drops, and the next call
/// makes a new state with `init` (4.7).
pub fn done_state<R, S>(caught: Caught<R>, state: &mut Option<S>, rep: &Reporter, node: u32, ok: &mut bool) -> Option<R> {
    let panicked = caught.is_err();
    let value = done(caught, rep, node, ok);
    if panicked {
        drop_value(state.take(), rep, node);
    }
    value
}

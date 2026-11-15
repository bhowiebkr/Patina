//! The worker pool of the scheduler (spec 9.2): one thread for each CPU core. The thread that
//! starts a run is one of them.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock, PoisonError, TryLockError};

/// The tasks of one run. A task starts when each task that it waits for is done.
pub trait Work: Send + Sync {
    /// The number of tasks that each task waits for.
    fn inputs(&self) -> &[u32];
    /// The tasks that wait for task `task`.
    fn next(&self, task: usize) -> &[usize];
    fn run(&self, task: usize);
}

struct State {
    job: Option<Arc<dyn Work>>,
    ready: Vec<usize>,
    left: Vec<u32>,
    pending: usize,
}

struct Pool {
    state: Mutex<State>,
    /// The pool notifies it when a task is ready, and when the last task of a run is done.
    wake: Condvar,
    /// The pool runs one run at a time.
    busy: Mutex<()>,
}

static POOL: OnceLock<Pool> = OnceLock::new();

/// Runs each task of `work` one time, and returns when all are done. If another run uses the
/// pool, this thread runs the tasks in index order, so the index order must keep the waits.
pub fn run(work: Arc<dyn Work>) {
    let pool = pool();
    let _busy = match pool.busy.try_lock() {
        Ok(guard) => guard,
        Err(TryLockError::Poisoned(e)) => e.into_inner(),
        Err(TryLockError::WouldBlock) => {
            (0..work.inputs().len()).for_each(|task| work.run(task));
            return;
        }
    };
    let mut state = lock(&pool.state);
    let inputs = work.inputs();
    state.left.clear();
    state.left.extend_from_slice(inputs);
    state.ready.clear();
    // Reverse order, so that the pops start at the first task.
    state.ready.extend((0..inputs.len()).rev().filter(|&t| inputs[t] == 0));
    state.pending = inputs.len();
    state.job = Some(work);
    for _ in 1..state.ready.len() {
        pool.wake.notify_one();
    }
    while state.pending > 0 {
        state = match state.ready.pop() {
            Some(task) => pool.step(state, task),
            None => pool.wake.wait(state).unwrap_or_else(PoisonError::into_inner),
        };
    }
    state.job = None;
}

fn pool() -> &'static Pool {
    let mut first = false;
    let pool = POOL.get_or_init(|| {
        first = true;
        let state = State { job: None, ready: Vec::new(), left: Vec::new(), pending: 0 };
        Pool { state: Mutex::new(state), wake: Condvar::new(), busy: Mutex::new(()) }
    });
    if first {
        let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
        for i in 1..cores {
            // A thread that does not start makes the pool smaller. The caller still runs tasks.
            let _ = std::thread::Builder::new().name(format!("patina-worker-{i}")).spawn(|| worker(pool));
        }
    }
    pool
}

fn worker(pool: &'static Pool) {
    let mut state = lock(&pool.state);
    loop {
        state = match state.ready.pop() {
            Some(task) => pool.step(state, task),
            None => pool.wake.wait(state).unwrap_or_else(PoisonError::into_inner),
        };
    }
}

impl Pool {
    /// Runs `task` with the lock open. Then marks it done and makes ready each task that waited
    /// only for it.
    fn step<'a>(&'a self, state: MutexGuard<'a, State>, task: usize) -> MutexGuard<'a, State> {
        let Some(job) = state.job.clone() else { return state };
        drop(state);
        let _ = catch_unwind(AssertUnwindSafe(|| job.run(task)));
        let mut state = lock(&self.state);
        let mut woke = 0;
        for &next in job.next(task) {
            state.left[next] -= 1;
            if state.left[next] == 0 {
                state.ready.push(next);
                woke += 1;
            }
        }
        state.pending -= 1;
        if state.pending == 0 {
            self.wake.notify_all();
        }
        // This thread runs one of the new tasks.
        for _ in 1..woke {
            self.wake.notify_one();
        }
        state
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Task 0, then tasks 1 and 2, then task 3. Task 1 starts a second run inside the first.
    struct Diamond {
        done: Mutex<Vec<usize>>,
        nested: bool,
    }

    impl Work for Diamond {
        fn inputs(&self) -> &[u32] {
            &[0, 1, 1, 2]
        }

        fn next(&self, task: usize) -> &[usize] {
            [&[1, 2][..], &[3], &[3], &[]][task]
        }

        fn run(&self, task: usize) {
            if task == 1 && self.nested {
                let inner = Arc::new(Diamond { done: Mutex::new(Vec::new()), nested: false });
                run(inner.clone());
                assert_eq!(*lock(&inner.done), [0, 1, 2, 3], "the inner run waits, in index order");
            }
            lock(&self.done).push(task);
        }
    }

    #[test]
    fn tasks_wait_for_their_inputs() {
        for _ in 0..3 {
            let work = Arc::new(Diamond { done: Mutex::new(Vec::new()), nested: true });
            run(work.clone());
            let done = lock(&work.done);
            assert_eq!((done.len(), done[0], done[3]), (4, 0, 3), "{done:?}");
        }
    }
}

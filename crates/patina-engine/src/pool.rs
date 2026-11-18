//! The worker pool of the scheduler (spec 9.2): one thread for each CPU core. The thread that
//! starts a run is one of them.

use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock, PoisonError, TryLockError};
use std::time::{Duration, Instant};

/// The time that the calling thread of a run spins before it sleeps. A sleeping thread wakes about
/// 60 us after the last task ends (9.2).
const SPIN: Duration = Duration::from_micros(200);

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
    /// The number of done tasks. It changes only with the lock, so a spin can watch it without.
    done: AtomicUsize,
}

static POOL: OnceLock<Pool> = OnceLock::new();

thread_local! {
    /// The index of this thread in the pool. The thread that starts a run, and a thread that is not
    /// in the pool, is 0.
    static INDEX: Cell<u32> = const { Cell::new(0) };
}

/// The index of this thread in the pool, for the timeline of the **Debug** tab (15.7).
pub fn thread() -> u32 {
    INDEX.get()
}

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
            None => pool.spin(state),
        };
    }
    state.job = None;
}

/// Starts the pool and makes room for runs of up to `tasks` tasks, so that a later run does not
/// allocate (9.1).
pub fn prepare(tasks: usize) {
    let mut state = lock(&pool().state);
    state.ready.reserve(tasks);
    state.left.reserve(tasks);
}

fn pool() -> &'static Pool {
    let mut first = false;
    let pool = POOL.get_or_init(|| {
        first = true;
        let state = State { job: None, ready: Vec::new(), left: Vec::new(), pending: 0 };
        Pool { state: Mutex::new(state), wake: Condvar::new(), busy: Mutex::new(()), done: AtomicUsize::new(0) }
    });
    if first {
        let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
        for i in 1..cores {
            let index = u32::try_from(i).unwrap_or(u32::MAX);
            // A thread that does not start makes the pool smaller. The caller still runs tasks.
            let _ = std::thread::Builder::new().name(format!("patina-worker-{i}")).spawn(move || worker(pool, index));
        }
    }
    pool
}

fn worker(pool: &'static Pool, index: u32) {
    INDEX.set(index);
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
        self.done.fetch_add(1, Relaxed);
        if state.pending == 0 {
            self.wake.notify_all();
        }
        // This thread runs one of the new tasks.
        for _ in 1..woke {
            self.wake.notify_one();
        }
        state
    }

    /// Waits until a task is done: first with a spin of up to `SPIN`, then on the condvar.
    fn spin<'a>(&'a self, state: MutexGuard<'a, State>) -> MutexGuard<'a, State> {
        let seen = self.done.load(Relaxed);
        drop(state);
        let end = Instant::now() + SPIN;
        while self.done.load(Relaxed) == seen && Instant::now() < end {
            std::hint::spin_loop();
        }
        let state = lock(&self.state);
        // A task ends only with the lock, so no end comes between this check and the wait.
        if self.done.load(Relaxed) != seen {
            return state;
        }
        self.wake.wait(state).unwrap_or_else(PoisonError::into_inner)
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

    /// Task 0 waits a little, and task 1 waits longer than the spin of the calling thread. Each
    /// task gives the pool index of its thread.
    struct Sleeps(Mutex<Vec<u32>>);

    impl Work for Sleeps {
        fn inputs(&self) -> &[u32] {
            &[0, 0]
        }

        fn next(&self, _: usize) -> &[usize] {
            &[]
        }

        fn run(&self, task: usize) {
            std::thread::sleep([SPIN / 2, SPIN * 10][task]);
            lock(&self.0).push(thread());
        }
    }

    #[test]
    fn a_run_ends_after_a_task_that_is_longer_than_the_spin() {
        let work = Arc::new(Sleeps(Mutex::new(Vec::new())));
        run(work.clone());
        assert_eq!(lock(&work.0).len(), 2);
    }

    /// The timeline of the **Debug** tab shows the pool index of each call (15.7).
    #[test]
    fn a_task_on_a_worker_gives_its_pool_index() {
        let cores = u32::try_from(std::thread::available_parallelism().map_or(1, |n| n.get())).unwrap_or(u32::MAX);
        // While another test holds the pool, this thread runs all the tasks, so try again.
        let on_worker = (0..100).any(|_| {
            let work = Arc::new(Sleeps(Mutex::new(Vec::new())));
            run(work.clone());
            let threads = lock(&work.0);
            assert!(threads.iter().all(|&t| t < cores), "{threads:?} with {cores} cores");
            threads.iter().any(|&t| t > 0)
        });
        assert_eq!(thread(), 0, "the thread of the run is 0");
        assert!(on_worker || cores == 1, "no task ran on a worker in 100 runs");
    }

    /// `n` tasks of `spins` loop steps each, in a chain or apart, or `n` x `spins` in one task.
    struct Load {
        inputs: Vec<u32>,
        next: Vec<Vec<usize>>,
        spins: u64,
    }

    impl Work for Load {
        fn inputs(&self) -> &[u32] {
            &self.inputs
        }

        fn next(&self, task: usize) -> &[usize] {
            &self.next[task]
        }

        fn run(&self, _: usize) {
            (0..self.spins).fold(0u64, |x, i| std::hint::black_box(x.wrapping_add(i)));
        }
    }

    /// The median time of one run of `work`, in microseconds.
    fn median(work: &Arc<Load>) -> f64 {
        let mut times: Vec<f64> = (0..201)
            .map(|_| {
                let start = std::time::Instant::now();
                run(work.clone());
                start.elapsed().as_secs_f64() * 1e6
            })
            .collect();
        times.sort_by(f64::total_cmp);
        times[100]
    }

    /// Spec 9.2: where to split. Run with `--release --ignored --nocapture`.
    #[test]
    #[ignore = "a measurement, not a check"]
    fn measure_split() {
        let n = 8;
        let chain = |spins| Load { inputs: (0..n).map(|t| u32::from(t > 0)).collect(), next: (0..n).map(|t| (t + 1..n).take(1).collect()).collect(), spins };
        let apart = |spins| Load { inputs: vec![0; n], next: vec![Vec::new(); n], spins };
        let one = |spins| Load { inputs: vec![0], next: vec![Vec::new()], spins: spins * n as u64 };
        println!("{n} tasks, {} cores. Times in us: one task, chain of {n}, {n} apart", std::thread::available_parallelism().map_or(1, |c| c.get()));
        for spins in [0, 100, 1_000, 10_000, 100_000, 1_000_000] {
            let [a, b, c] = [one(spins), chain(spins), apart(spins)].map(|w| median(&Arc::new(w)));
            println!("{spins:>9} spins: {a:>9.1} {b:>9.1} {c:>9.1}");
        }
    }
}

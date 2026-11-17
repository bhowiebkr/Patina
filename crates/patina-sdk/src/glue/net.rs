//! A net: plain light nodes in a row of one task (spec 9.2). Their calls share one **Stop** check,
//! one catch and one clock read, so a gate costs about as much as its own code.

use std::cell::Cell;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicU32, Ordering::Relaxed};
use std::time::Instant;

use super::{NODE, PANIC, Reporter, SKIPPED, STOPPED, catch_quiet, panic_text};

/// The node index of a net in a call that is not a run, which nobody reads.
static NO_CONTROL: AtomicU32 = AtomicU32::new(0);

/// Records the node of each gate of a net before its call: in the thread-local value for the log
/// lines (15.7), and in the run control for a node that does not end (15.2). The closure of the
/// net gets it as an argument, so the compiler keeps the wires of the net in registers.
#[derive(Clone, Copy)]
pub struct Gate<'a> {
    node: &'a Cell<u32>,
    control: &'a AtomicU32,
}

impl Gate<'_> {
    /// Records that node `node` starts.
    #[inline(always)]
    pub fn enter(self, node: u32) {
        self.node.set(node);
        self.control.store(node, Relaxed);
    }
}

/// The run of one net. `nodes` holds the node index of each call, in run order.
pub struct Net<'a> {
    rep: &'a Reporter,
    nodes: &'a [u32],
    node: Option<&'a AtomicU32>,
    start: Option<Instant>,
}

impl<'a> Net<'a> {
    /// Starts a net. After **Stop** (15.2), it reports `STOPPED` for each node and gives None.
    pub fn new(rep: &'a Reporter, nodes: &'a [u32]) -> Option<Net<'a>> {
        let control = rep.control();
        if control.is_some_and(super::Control::stopped) {
            for &node in nodes {
                rep.send(node, STOPPED, "");
            }
            return None;
        }
        // A quiet run sends no times (9.1), so it does not read the clock.
        Some(Net { rep, nodes, node: control.map(|c| &c.node), start: (!rep.quiet()).then(Instant::now) })
    }

    /// Reports that node `node` did not start, because a wired input had no value (15.3).
    #[cold]
    #[inline(never)]
    pub fn skip(&self, node: u32) {
        self.rep.send(node, SKIPPED, "");
    }

    /// Runs `calls` in one catch (4.7). `calls` runs call `at` and the calls after it, and calls
    /// `gate.enter` before each call starts. After a panic, it reports the panic of the call that
    /// entered last, and runs again from the next call. Each node then gets the result of its own
    /// catch. Gives false when a call panicked.
    pub fn run(&self, calls: &mut dyn FnMut(usize, Gate)) -> bool {
        let (mut at, mut ok) = (0, true);
        NODE.with(|node| {
            let gate = Gate { node, control: self.node.unwrap_or(&NO_CONTROL) };
            while at < self.nodes.len() {
                match catch_quiet(AssertUnwindSafe(|| calls(at, gate))) {
                    Ok(()) => break,
                    Err(payload) => {
                        // A call from `at` panicked, so the search starts at `at`, and the next run starts after it.
                        let cur = self.nodes[at..].iter().position(|&n| n == node.get()).map_or(at, |i| at + i);
                        self.rep.send(self.nodes[cur], PANIC, &panic_text(&*payload));
                        ok = false;
                        at = cur + 1;
                    }
                }
            }
        });
        // Each node gets an equal part of the time of the net, so the sum stays right (9.2).
        if let Some(start) = self.start {
            let each = start.elapsed().as_nanos() / self.nodes.len() as u128;
            for &node in self.nodes {
                self.rep.send_time(node, each);
            }
        }
        ok
    }
}

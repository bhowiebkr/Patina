//! A net: plain light nodes in a row of one task (spec 9.2). Their calls share one **Stop** check,
//! one catch and one clock read, so a gate costs about as much as its own code.

use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicU32, Ordering::Relaxed};
use std::time::Instant;

use super::{NODE, PANIC, Reporter, SKIPPED, STOPPED, catch_quiet, panic_text};

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

    /// Records that node `node` starts, for the log lines (15.7) and a node that does not end.
    #[inline(always)]
    pub fn enter(&self, node: u32) {
        NODE.set(node);
        if let Some(control) = self.node {
            control.store(node, Relaxed);
        }
    }

    /// Reports that node `node` did not start, because a wired input had no value (15.3).
    #[cold]
    #[inline(never)]
    pub fn skip(&self, node: u32) {
        self.rep.send(node, SKIPPED, "");
    }

    /// Runs `calls` in one catch (4.7). `calls` runs call `at` and the calls after it, and sets
    /// `cur` to each call before it starts. After a panic, it reports the panic of call `cur`, and
    /// runs again from the next call. Each node then gets the result of its own catch. Gives false
    /// when a call panicked.
    pub fn run(&self, calls: &mut dyn FnMut(usize, &mut usize)) -> bool {
        let (mut at, mut ok) = (0, true);
        while at < self.nodes.len() {
            let mut cur = at;
            match catch_quiet(AssertUnwindSafe(|| calls(at, &mut cur))) {
                Ok(()) => break,
                Err(payload) => {
                    self.rep.send(self.nodes[cur], PANIC, &panic_text(&*payload));
                    ok = false;
                    at = cur + 1;
                }
            }
        }
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

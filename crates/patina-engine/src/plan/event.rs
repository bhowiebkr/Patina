//! Checks the event domain of a graph (spec 9.4): each event node gets its runs from one trigger.

use std::collections::BTreeSet;

use super::Step;
use crate::node::Domain;

/// The trigger of each step: the step itself for a trigger, the one trigger that reaches an event
/// step through its event predecessors, or None outside the event domain. `fires(n)` is true for
/// a trigger. The steps are in run order, so each predecessor has its trigger before the step.
pub(super) fn triggers(steps: &[Step], edges: &[(usize, usize)], fires: impl Fn(usize) -> bool) -> Result<Vec<Option<usize>>, Vec<String>> {
    let mut problems = Vec::new();
    let mut trigger = vec![None; steps.len()];
    for n in 0..steps.len() {
        let before = || edges.iter().filter(move |e| e.1 == n).map(|e| e.0);
        let id = &steps[n].id;
        if fires(n) {
            if before().any(|p| steps[p].domain == Domain::Event) {
                problems.push(format!("node {id}: a trigger starts its runs, so no event node comes before it (spec 9.4)"));
            }
            trigger[n] = Some(n);
        } else if steps[n].domain == Domain::Event {
            let found: BTreeSet<usize> = before().filter_map(|p| trigger[p]).collect();
            match found.len() {
                1 => trigger[n] = found.first().copied(),
                0 => problems.push(format!("node {id}: connect a wire or an order wire from a trigger node: schedule, trigger, file_change or webhook (spec 9.4)")),
                _ => problems.push(format!("node {id}: two triggers reach this node. Connect one trigger (spec 9.4)")),
            }
        }
    }
    if problems.is_empty() { Ok(trigger) } else { Err(problems) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_event_step_has_one_trigger() {
        let domains = [Domain::Batch, Domain::Event, Domain::Event, Domain::Event, Domain::Event, Domain::Event, Domain::Event];
        let steps: Vec<Step> = (domains.iter().enumerate())
            .map(|(n, &domain)| Step { id: n.to_string(), node: 0, inputs: Vec::new(), consumers: Vec::new(), types: Vec::new(), pins: Vec::new(), task: 0, domain })
            .collect();
        // Steps 1 and 3 are triggers. Batch 0 and trigger 1 feed 2.
        let fires = |n: usize| n == 1 || n == 3;
        let good = triggers(&steps[..4], &[(0, 2), (1, 2)], fires);
        assert_eq!(good, Ok(vec![None, Some(1), Some(1), Some(3)]));
        let bad = triggers(&steps, &[(0, 2), (1, 2), (2, 3), (2, 4), (3, 4), (2, 6)], fires).unwrap_err();
        assert_eq!(bad.len(), 3, "{bad:?}");
        assert!(bad[0].starts_with("node 3: a trigger starts its runs"), "{bad:?}");
        assert!(bad[1].starts_with("node 4: two triggers"), "{bad:?}");
        assert!(bad[2].starts_with("node 5: connect a wire"), "{bad:?}");
    }
}

//! Sets the domain of each step of a graph (spec 9.1, 9.2).

use std::collections::BTreeSet;

use super::{Source, Step};
use crate::node::Domain;

/// Sets the domain of each step. `chosen[n]` is the domain that the graph file gives step n, and
/// `is_delay` takes a node index. A delay with no chosen domain runs in the audio domain when its
/// input comes from there, else in the frame domain (9.2).
pub(super) fn assign(steps: &mut [Step], edges: &[(usize, usize)], chosen: &[Option<Domain>], is_delay: impl Fn(usize) -> bool) -> Result<(), Vec<String>> {
    let mut own = chosen.to_vec();
    for n in (0..steps.len()).filter(|&n| is_delay(steps[n].node)) {
        own[n] = own[n].or(Some(Domain::Frame));
    }
    loop {
        domains(steps, edges, |n| own[n])?;
        let audio = (0..steps.len()).find(|&n| match steps[n].inputs.first() {
            Some(&Source::Wire { step, .. }) if is_delay(steps[n].node) => chosen[n].is_none() && own[n] != Some(Domain::Audio) && steps[step].domain == Domain::Audio,
            _ => false,
        });
        let Some(n) = audio else { break };
        own[n] = Some(Domain::Audio);
    }
    for step in steps.iter().filter(|s| is_delay(s.node)) {
        let Source::Wire { step: from, .. } = step.inputs[0] else { continue };
        if !matches!(step.domain, Domain::Frame | Domain::Audio) {
            return Err(vec![format!("node {}: a delay runs in the frame or the audio domain, not in the {} domain", step.id, step.domain.name())]);
        }
        if !reaches(steps[from].domain, step.domain) {
            return Err(vec![format!("node {}: the delay reads the {} domain. Connect a value of the frame or the audio domain", step.id, steps[from].domain.name())]);
        }
    }
    Ok(())
}

/// True when a wire can go from domain `from` to domain `to` (9.1). The batch domain runs before
/// the other domains, so they read the latest batch value. No value goes back to batch. The frame
/// and the audio domain run at the same time and read the latest values of each other. In version
/// 1, only the batch domain gives values to the event domain, and no value leaves it.
pub(super) fn reaches(from: Domain, to: Domain) -> bool {
    let event = from == Domain::Event || to == Domain::Event;
    from == to || from == Domain::Batch || (to != Domain::Batch && !event)
}

/// Sets the domain of each step (9.1). A step with no domain of its own takes the domain that all
/// its inputs reach, audio before frame before batch, or the batch domain when it has no inputs.
/// A wire back in a cycle of gates (9.2) reads the domain of the pass before, so the passes repeat
/// until no domain changes.
fn domains(steps: &mut [Step], edges: &[(usize, usize)], own: impl Fn(usize) -> Option<Domain>) -> Result<(), Vec<String>> {
    let mut problems = Vec::new();
    let back = edges.iter().any(|&(from, to)| from >= to);
    for _ in 0..=steps.len() {
        problems.clear();
        let mut moved = false;
        for n in 0..steps.len() {
            let inputs: BTreeSet<Domain> = edges.iter().filter(|e| e.1 == n).map(|e| steps[e.0].domain).collect();
            let common = inputs.iter().copied().rev().find(|&d| inputs.iter().all(|&i| reaches(i, d)));
            let domain = match (own(n), common) {
                (Some(domain), _) | (None, Some(domain)) => domain,
                (None, None) if inputs.is_empty() => Domain::Batch,
                (None, None) => {
                    problems.push(format!("node {}: its inputs come from more than one domain. Select its domain", steps[n].id));
                    Domain::Batch
                }
            };
            moved |= steps[n].domain != domain;
            steps[n].domain = domain;
        }
        if !back || !moved {
            break;
        }
    }
    for &(from, to) in edges.iter().collect::<BTreeSet<_>>() {
        let (a, b) = (steps[from].domain, steps[to].domain);
        if !reaches(a, b) && own(to).is_some() {
            problems.push(format!("node {}: a value cannot go from the {} domain to the {} domain", steps[to].id, a.name(), b.name()));
        }
    }
    if problems.is_empty() { Ok(()) } else { Err(problems) }
}

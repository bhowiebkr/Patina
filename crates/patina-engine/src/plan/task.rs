//! Puts the steps of a checked graph in run order and into tasks (spec 9.2).

use std::collections::{BTreeMap, BTreeSet};

use super::{Source, Step, Task, scalar};
use crate::node::{self, Domain, Node};

/// Kahn's algorithm. Ties go in file order. Returns the nodes of a cycle on failure.
pub(super) fn sort(count: usize, edges: &[(usize, usize)]) -> Result<Vec<usize>, Vec<usize>> {
    let mut indegree = vec![0usize; count];
    let mut after: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for &(from, to) in edges {
        indegree[to] += 1;
        after.entry(from).or_default().push(to);
    }
    let mut ready: BTreeSet<usize> = (0..count).filter(|&i| indegree[i] == 0).collect();
    let mut order = Vec::with_capacity(count);
    while let Some(next) = ready.pop_first() {
        order.push(next);
        for &to in after.get(&next).into_iter().flatten() {
            indegree[to] -= 1;
            if indegree[to] == 0 {
                ready.insert(to);
            }
        }
    }
    if order.len() == count {
        Ok(order)
    } else {
        Err((0..count).filter(|&i| indegree[i] > 0).collect())
    }
}

/// True when step `n` is so small that a task of its own costs more than the step (9.2): a delay,
/// or a node with only `bool`, `char` and number values, for example a NAND gate. `heavy` stops it.
pub(super) fn is_light(steps: &[Step], nodes: &[(String, Node)], n: usize, is_delay: bool) -> bool {
    let (step, node) = (&steps[n], &nodes[steps[n].node].1);
    let input = |(i, source): (&node::Input, &Source)| match *source {
        Source::Wire { step, output } => scalar(&steps[step].types[output]),
        Source::Slot(_) => scalar(&i.value),
    };
    is_delay
        || (!node.is_async
            && !node.heavy
            && step.domain != Domain::Event
            && node.inputs.iter().zip(&step.inputs).all(input)
            && step.types.iter().take(step.consumers.len()).all(|ty| scalar(ty)))
}

/// Puts the steps into tasks (9.2). A light step joins the newest light task of its domain when
/// each of its predecessors is in that task or an older one. A task index is then never smaller
/// than the index of a task before it. Another step joins the task of its predecessor when it has
/// exactly one predecessor, that predecessor has exactly one successor, and both have the same
/// domain. An event step joins the task of its trigger, so an event run is one call (9.4).
pub(super) fn split(steps: &mut [Step], edges: &[(usize, usize)], trigger: &[Option<usize>], light: &[bool]) -> Vec<Task> {
    let mut before: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); steps.len()];
    let mut after: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); steps.len()];
    for &(from, to) in edges {
        before[to].insert(from);
        after[from].insert(to);
    }
    let mut tasks: Vec<Task> = Vec::new();
    let mut open: BTreeMap<Domain, usize> = BTreeMap::new();
    for n in 0..steps.len() {
        let chain = before[n].first().filter(|&&p| before[n].len() == 1 && after[p].len() == 1 && steps[p].domain == steps[n].domain);
        let joined = match trigger[n] {
            Some(t) if t != n => Some(steps[t].task),
            _ if light[n] => open.get(&steps[n].domain).copied().filter(|&t| before[n].iter().all(|&p| steps[p].task <= t)),
            _ => chain.map(|&p| steps[p].task),
        };
        steps[n].task = match joined {
            Some(task) => task,
            None => {
                tasks.push(Task { steps: Vec::new(), inputs: 0, next: Vec::new(), domain: steps[n].domain });
                if light[n] {
                    open.insert(steps[n].domain, tasks.len() - 1);
                }
                tasks.len() - 1
            }
        };
        tasks[steps[n].task].steps.push(n);
    }
    let links: BTreeSet<(usize, usize)> = edges.iter().map(|&(from, to)| (steps[from].task, steps[to].task)).filter(|(a, b)| a != b).collect();
    for (from, to) in links {
        tasks[from].next.push(to);
        tasks[to].inputs += 1;
    }
    tasks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sort_keeps_file_order_and_finds_cycles() {
        assert_eq!(sort(3, &[(2, 0)]), Ok(vec![1, 2, 0]));
        assert_eq!(sort(3, &[(0, 1), (1, 0)]), Err(vec![0, 1]));
    }

    #[test]
    fn chains_become_one_task_and_branches_split() {
        let mut steps: Vec<Step> = (0..6).map(|_| Step { id: String::new(), node: 0, inputs: Vec::new(), consumers: Vec::new(), types: Vec::new(), pins: Vec::new(), task: 0, domain: Domain::Batch }).collect();
        // 0 -> 1 -> 2 is a chain. 2 feeds 3 and 4, and both feed 5. An order wire 1 -> 2 doubles a data wire.
        let tasks = split(&mut steps, &[(0, 1), (1, 2), (1, 2), (2, 3), (2, 4), (3, 5), (4, 5)], &[None; 6], &[false; 6]);
        let parts: Vec<(&[usize], u32, &[usize])> = tasks.iter().map(|t| (&t.steps[..], t.inputs, &t.next[..])).collect();
        assert_eq!(parts, [(&[0, 1, 2][..], 0, &[1, 2][..]), (&[3], 1, &[3]), (&[4], 1, &[3]), (&[5], 2, &[])]);
        assert_eq!(steps.iter().map(|s| s.task).collect::<Vec<_>>(), [0, 0, 0, 1, 2, 3]);
    }

    #[test]
    fn light_steps_share_a_task_after_the_tasks_they_read() {
        let mut steps: Vec<Step> = (0..7).map(|_| Step { id: String::new(), node: 0, inputs: Vec::new(), consumers: Vec::new(), types: Vec::new(), pins: Vec::new(), task: 0, domain: Domain::Batch }).collect();
        steps[6].domain = Domain::Frame;
        // Step 2 is heavy. Step 4 reads it, so 4 cannot join the older light task. Step 6 is in another domain.
        let light = [true, true, false, true, true, true, true];
        let tasks = split(&mut steps, &[(0, 1), (0, 2), (1, 3), (2, 4), (3, 5), (4, 5), (5, 6)], &[None; 7], &light);
        let parts: Vec<(&[usize], u32, &[usize])> = tasks.iter().map(|t| (&t.steps[..], t.inputs, &t.next[..])).collect();
        assert_eq!(parts, [(&[0, 1, 3][..], 0, &[1, 2][..]), (&[2], 1, &[2]), (&[4, 5], 2, &[3]), (&[6], 1, &[])]);
    }
}

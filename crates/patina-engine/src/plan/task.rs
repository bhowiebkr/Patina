//! Puts the steps of a checked graph in run order and into tasks (spec 9.2).

use std::collections::{BTreeMap, BTreeSet};

use super::{Source, Step, Task, scalar};
use crate::node::{self, Domain, Node};

/// Puts the nodes in run order (9.2). The nodes of a cycle are one group, and the groups go in
/// the order of their wires, with ties in file order. In a group, a node comes after the nodes of
/// the group that it reads, when it can. Returns the order and the inclusive range in the order of
/// each group that is a cycle.
pub(super) fn sort(count: usize, edges: &[(usize, usize)]) -> (Vec<usize>, Vec<(usize, usize)>) {
    let mut after = vec![Vec::new(); count];
    for &(from, to) in edges {
        after[from].push(to);
    }
    let group = groups(&after);
    let mut members: Vec<Vec<usize>> = vec![Vec::new(); group.iter().max().map_or(0, |g| g + 1)];
    for (n, &g) in group.iter().enumerate() {
        members[g].push(n);
    }
    let (mut indegree, mut pending, mut looped) = (vec![0usize; members.len()], vec![0usize; count], vec![false; members.len()]);
    for &(from, to) in edges {
        match (group[from], group[to]) {
            (a, b) if a != b => indegree[b] += 1,
            (a, _) if from == to => looped[a] = true,
            _ => pending[to] += 1,
        }
    }
    let mut ready: BTreeSet<(usize, usize)> = (0..members.len()).filter(|&g| indegree[g] == 0).map(|g| (members[g][0], g)).collect();
    let (mut order, mut loops) = (Vec::with_capacity(count), Vec::new());
    while let Some((_, g)) = ready.pop_first() {
        let start = order.len();
        let mut left: BTreeSet<usize> = members[g].iter().copied().collect();
        let mut free: BTreeSet<usize> = members[g].iter().copied().filter(|&m| pending[m] == 0).collect();
        while let Some(&first) = left.first() {
            // When each node left reads another node left, the first node left breaks the cycle.
            let next = free.pop_first().unwrap_or(first);
            left.remove(&next);
            order.push(next);
            for &to in &after[next] {
                if group[to] == g && to != next && left.contains(&to) {
                    pending[to] -= 1;
                    if pending[to] == 0 {
                        free.insert(to);
                    }
                }
            }
        }
        if looped[g] || members[g].len() > 1 {
            loops.push((start, order.len() - 1));
        }
        for &n in &members[g] {
            for &to in &after[n] {
                let h = group[to];
                if h != g {
                    indegree[h] -= 1;
                    if indegree[h] == 0 {
                        ready.insert((members[h][0], h));
                    }
                }
            }
        }
    }
    (order, loops)
}

/// The group of each node: the nodes that reach each other through `after` share a group. This is
/// the algorithm of Tarjan, with a stack of its own, because a large graph can overflow the stack
/// of the thread.
fn groups(after: &[Vec<usize>]) -> Vec<usize> {
    const NONE: usize = usize::MAX;
    let count = after.len();
    let (mut index, mut low, mut group) = (vec![NONE; count], vec![0; count], vec![NONE; count]);
    let (mut next, mut found, mut stack, mut calls) = (0, 0, Vec::new(), Vec::new());
    for root in 0..count {
        if index[root] != NONE {
            continue;
        }
        (index[root], low[root], next) = (next, next, next + 1);
        stack.push(root);
        calls.push((root, 0));
        while let Some(top) = calls.last_mut() {
            let (v, i) = *top;
            if let Some(&w) = after[v].get(i) {
                top.1 += 1;
                if index[w] == NONE {
                    (index[w], low[w], next) = (next, next, next + 1);
                    stack.push(w);
                    calls.push((w, 0));
                } else if group[w] == NONE {
                    low[v] = low[v].min(index[w]);
                }
                continue;
            }
            calls.pop();
            if let Some(&(parent, _)) = calls.last() {
                low[parent] = low[parent].min(low[v]);
            }
            if low[v] == index[v] {
                while let Some(w) = stack.pop() {
                    group[w] = found;
                    if w == v {
                        break;
                    }
                }
                found += 1;
            }
        }
    }
    group
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
/// domain. An event step joins the task of its trigger, so an event run is one call (9.4). The
/// light steps of each range of `loops` are one cycle (9.2). They are one task, and the first step
/// of the range waits for the predecessors of all of them.
pub(super) fn split(steps: &mut [Step], edges: &[(usize, usize)], trigger: &[Option<usize>], light: &[bool], loops: &[(usize, usize)]) -> Vec<Task> {
    let mut before: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); steps.len()];
    let mut after: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); steps.len()];
    for &(from, to) in edges.iter().filter(|(from, to)| from < to) {
        before[to].insert(from);
        after[from].insert(to);
    }
    for &(first, last) in loops {
        let outside: BTreeSet<usize> = (first..=last).flat_map(|m| before[m].iter().copied()).filter(|&p| p < first).collect();
        before[first] = outside;
    }
    let mut tasks: Vec<Task> = Vec::new();
    let mut open: BTreeMap<Domain, usize> = BTreeMap::new();
    for n in 0..steps.len() {
        let chain = before[n].first().filter(|&&p| before[n].len() == 1 && after[p].len() == 1 && steps[p].domain == steps[n].domain);
        let joined = match trigger[n] {
            Some(t) if t != n => Some(steps[t].task),
            _ if let Some(&(first, _)) = loops.iter().find(|&&(first, last)| first < n && n <= last) => Some(steps[first].task),
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
    fn sort_keeps_file_order_and_groups_cycles() {
        assert_eq!(sort(3, &[(2, 0)]), (vec![1, 2, 0], vec![]));
        // 1 and 2 read each other and 3 reads itself. 4 reads 2 and 3, so it comes after both groups.
        assert_eq!(sort(5, &[(0, 1), (1, 2), (2, 1), (2, 4), (3, 3), (3, 4)]), (vec![0, 1, 2, 3, 4], vec![(1, 2), (3, 3)]));
        // In the group 0 -> 2 -> 1 -> 0, each node reads another, so 0 breaks the cycle and 2, 1 follow their wires.
        assert_eq!(sort(4, &[(3, 0), (0, 2), (2, 1), (1, 0)]), (vec![3, 0, 2, 1], vec![(1, 3)]));
        // The duplicate wire 2 -> 0 counts two times, and 0 is placed before its second count.
        assert_eq!(sort(3, &[(1, 2), (2, 0), (2, 0), (0, 2)]), (vec![1, 0, 2], vec![(1, 2)]));
    }

    #[test]
    fn chains_become_one_task_and_branches_split() {
        let mut steps: Vec<Step> = (0..6).map(|_| Step { id: String::new(), node: 0, inputs: Vec::new(), consumers: Vec::new(), readers: Vec::new(), types: Vec::new(), pins: Vec::new(), task: 0, domain: Domain::Batch }).collect();
        // 0 -> 1 -> 2 is a chain. 2 feeds 3 and 4, and both feed 5. An order wire 1 -> 2 doubles a data wire.
        let tasks = split(&mut steps, &[(0, 1), (1, 2), (1, 2), (2, 3), (2, 4), (3, 5), (4, 5)], &[None; 6], &[false; 6], &[]);
        let parts: Vec<(&[usize], u32, &[usize])> = tasks.iter().map(|t| (&t.steps[..], t.inputs, &t.next[..])).collect();
        assert_eq!(parts, [(&[0, 1, 2][..], 0, &[1, 2][..]), (&[3], 1, &[3]), (&[4], 1, &[3]), (&[5], 2, &[])]);
        assert_eq!(steps.iter().map(|s| s.task).collect::<Vec<_>>(), [0, 0, 0, 1, 2, 3]);
    }

    #[test]
    fn light_steps_share_a_task_after_the_tasks_they_read() {
        let mut steps: Vec<Step> = (0..7).map(|_| Step { id: String::new(), node: 0, inputs: Vec::new(), consumers: Vec::new(), readers: Vec::new(), types: Vec::new(), pins: Vec::new(), task: 0, domain: Domain::Batch }).collect();
        steps[6].domain = Domain::Frame;
        // Step 2 is heavy. Step 4 reads it, so 4 cannot join the older light task. Step 6 is in another domain.
        let light = [true, true, false, true, true, true, true];
        let tasks = split(&mut steps, &[(0, 1), (0, 2), (1, 3), (2, 4), (3, 5), (4, 5), (5, 6)], &[None; 7], &light, &[]);
        let parts: Vec<(&[usize], u32, &[usize])> = tasks.iter().map(|t| (&t.steps[..], t.inputs, &t.next[..])).collect();
        assert_eq!(parts, [(&[0, 1, 3][..], 0, &[1, 2][..]), (&[2], 1, &[2]), (&[4, 5], 2, &[3]), (&[6], 1, &[])]);
    }

    #[test]
    fn a_cycle_is_one_task_that_waits_for_the_inputs_of_all_its_steps() {
        let mut steps: Vec<Step> = (0..4).map(|_| Step { id: String::new(), node: 0, inputs: Vec::new(), consumers: Vec::new(), readers: Vec::new(), types: Vec::new(), pins: Vec::new(), task: 0, domain: Domain::Frame }).collect();
        // 2 and 3 read each other. 2 reads 0, and 3 reads the heavy 1, so 2 cannot join the light task of 0.
        let tasks = split(&mut steps, &[(0, 2), (1, 3), (2, 3), (3, 2)], &[None; 4], &[true, false, true, true], &[(2, 3)]);
        let parts: Vec<(&[usize], u32, &[usize])> = tasks.iter().map(|t| (&t.steps[..], t.inputs, &t.next[..])).collect();
        assert_eq!(parts, [(&[0][..], 0, &[2][..]), (&[1], 0, &[2]), (&[2, 3], 2, &[])]);
    }
}

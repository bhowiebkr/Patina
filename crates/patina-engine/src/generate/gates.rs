//! The gate nets of a task (9.2): the wires back in a cycle of gates, and the block of a net
//! with its fast copy and its slow copy.
use std::collections::BTreeSet;
use std::fmt::Write;

use super::{held, is_kept, keep, send};
use crate::node::{Output, Pass};
use crate::plan::{Plan, Source};

/// The outputs that the wires back in the cycle from step `first` to step `last` read (9.2).
pub(super) fn backs(plan: &Plan, first: usize, last: usize) -> BTreeSet<(usize, usize)> {
    let back = |n: usize| {
        plan.steps[n].inputs.iter().filter_map(move |source| match *source {
            Source::Wire { step: from, output } if from >= n => Some((from, output)),
            _ => None,
        })
    };
    (first..=last).flat_map(back).collect()
}

/// Ends the run of the cycle of gates from step `first` to step `last` (9.2). Each wire back keeps
/// the newest value for the next frame or block, as a pin of Digital Logic Sim 2 does.
pub(super) fn save_backs(s: &mut String, plan: &Plan, first: usize, last: usize) {
    for (from, k) in backs(plan, first, last) {
        let _ = writeln!(
            s,
            "            if let ::std::option::Option::Some(v) = {}.as_ref() {{ *b{from}_{k}.write() = *v; }}",
            held(plan, from, k)[0]
        );
    }
}

/// True when step `n` can be a gate of a net (9.2): a light node with no state, no error, no
/// `&mut` input, number parameters and one output, which no other domain reads.
pub(super) fn is_gate(plan: &Plan, n: usize) -> bool {
    let node = &plan.nodes[plan.steps[n].node].1;
    plan.is_light(n)
        && node.inputs.iter().zip(&plan.steps[n].inputs).all(|(i, source)| !matches!(source, Source::Slot(_)) || crate::plan::scalar(&i.value))
        && plan.builtin(n).is_none()
        && !plan.in_slot(n)
        && node.state.is_none()
        && !node.fallible
        && node.output == Output::One
        && node.inputs.iter().all(|i| i.pass != Pass::Mut)
        && !is_kept(plan, n, 0)
}

/// Writes the gates of `net`, which are in a row in one task, in one block (9.2). The block has
/// one **Stop** check, one catch and one clock read. The values are numbers, so a gate copies its
/// inputs, and the probes of the net come after the block (15.9). The fast copy of the block runs
/// first. After a panic, the slow copy runs again from the next gate, so each gate gets the result
/// of its own catch (4.7, 15.3).
pub(super) fn net(s: &mut String, plan: &Plan, net: &[usize], first: &[usize]) {
    let nodes: Vec<String> = net.iter().map(usize::to_string).collect();
    let _ = writeln!(s, "            if let ::std::option::Option::Some(net) = ::patina_sdk::glue::Net::new(rep, &[{}]) {{", nodes.join(", "));
    s.push_str("                ok &= net.run(&mut |at, gate| {
");
    fast(s, plan, net);
    for (i, &n) in net.iter().enumerate() {
        let step = &plan.steps[n];
        let (name, node) = &plan.nodes[step.node];
        let (mut pats, mut wired, mut args) = (Vec::new(), Vec::new(), Vec::new());
        for (input, source) in node.inputs.iter().zip(&step.inputs) {
            let value = match *source {
                Source::Slot(p) => format!("*p{p}.read()"),
                Source::Wire { step: from, output } if plan.is_back(n, from) => format!("*b{from}_{output}.read()"),
                Source::Wire { step: from, output } => {
                    pats.push(format!("::std::option::Option::Some(a{})", wired.len()));
                    wired.push(format!("{}.as_ref()", held(plan, from, output)[0]));
                    format!("*a{}", wired.len() - 1)
                }
            };
            args.push(if input.pass == Pass::Ref { format!("&{value}") } else { value });
        }
        let call = format!("::{}::{}({})", name.replace('-', "_"), node.func, args.join(", "));
        let run = if step.consumers[0] > 0 {
            format!("gate.enter({n}); {} = ::std::option::Option::Some({call});", held(plan, n, 0)[2])
        } else {
            format!("gate.enter({n}); let _ = {call};")
        };
        let run = if wired.is_empty() {
            run
        } else {
            format!("if let ({},) = ({},) {{ {run} }} else {{ net.skip({n}); }}", pats.join(", "), wired.join(", "))
        };
        let at = if i == 0 { "at == 0".to_owned() } else { format!("at <= {i}") };
        let _ = writeln!(s, "                    if {at} {{ {run} }} // {}: nodes/{name}", step.id);
    }
    s.push_str("                });\n");
    // A gate has one output. One check of `quiet` and one of `any` cover the probes of the net.
    // A view reads only between two ticks, so a quiet frame keeps no copies either (15.8).
    let shown: Vec<usize> = net.iter().copied().filter(|&n| plan.steps[n].consumers[0] > 0).collect();
    if !shown.is_empty() {
        s.push_str("                if !rep.quiet() {\n");
        for &n in &shown {
            let _ = writeln!(s, "                    if let ::std::option::Option::Some(v) = {}.as_ref() {{ {} }}", held(plan, n, 0)[0], send(plan, n, 0, first[n]));
        }
        s.push_str("                    if keeps.any() {\n");
        for &n in &shown {
            let _ = writeln!(s, "                        if let ::std::option::Option::Some(v) = {}.as_ref() {{ {} }}", held(plan, n, 0)[0], keep(first[n]));
        }
        s.push_str("                    }\n                }\n");
    }
    s.push_str("            }
");
}

/// Writes the fast copy of `net` (9.2). When each wire from outside the net has a value, no gate
/// can get an input with no value, so the gates pass plain values with no `Option` check. They
/// also store each output, so after a panic the slow copy continues from the next gate.
fn fast(s: &mut String, plan: &Plan, net: &[usize]) {
    let (mut outside, mut gates) = (Vec::new(), String::new());
    for &n in net {
        let step = &plan.steps[n];
        let (name, node) = &plan.nodes[step.node];
        let mut args = Vec::with_capacity(node.inputs.len());
        for (input, source) in node.inputs.iter().zip(&step.inputs) {
            let value = match *source {
                Source::Slot(p) => format!("*p{p}.read()"),
                Source::Wire { step: from, output } if plan.is_back(n, from) => format!("*b{from}_{output}.read()"),
                // A gate has one output, and the gates of a task are in the order of their wires.
                Source::Wire { step: from, .. } if net.contains(&from) => format!("v{from}"),
                Source::Wire { step: from, output } => {
                    let i = outside.iter().position(|&w| w == (from, output)).unwrap_or(outside.len());
                    if i == outside.len() {
                        outside.push((from, output));
                    }
                    format!("e{i}")
                }
            };
            args.push(if input.pass == Pass::Ref { format!("&{value}") } else { value });
        }
        let call = format!("::{}::{}({})", name.replace('-', "_"), node.func, args.join(", "));
        let _ = if step.consumers[0] > 0 {
            writeln!(gates, "                        gate.enter({n}); let v{n} = {call}; {} = ::std::option::Option::Some(v{n});", held(plan, n, 0)[2])
        } else {
            writeln!(gates, "                        gate.enter({n}); let _ = {call};")
        };
    }
    if outside.is_empty() {
        let _ = writeln!(s, "                    if at == 0 {{\n{gates}                        return;\n                    }}");
        return;
    }
    // The values are copied first, so a read lock does not stay for the gates.
    let (mut pats, mut reads) = (String::new(), String::new());
    for (i, &(from, output)) in outside.iter().enumerate() {
        let _ = write!(pats, "::std::option::Option::Some(e{i}), ");
        let _ = write!(reads, "{}.as_ref().copied(), ", held(plan, from, output)[0]);
    }
    let _ = writeln!(
        s,
        "                    if at == 0 {{\n                        let wires = ({reads});\n                        if let ({pats}) = wires {{\n{gates}                        return;\n                        }}\n                    }}"
    );
}

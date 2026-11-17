//! Writes the cluster crates of a plan (spec 8.2): one crate for each clock domain.
//!
//! The code names no type of a node crate: the parameters live in one closure, and Rust infers
//! each type from the node calls. The code uses only full paths, because a node crate can have
//! the name of any `use`.

use std::collections::BTreeSet;
use std::fmt::Write;

use crate::node::{Builtin, Domain, Output, Pass};
use crate::plan::{Plan, Source};

mod exports;
pub mod slot;

use exports::{EXPORTS, LOGGER};

const HEADER: &str = "# Made by Patina. Patina writes this file again at each build.\n";

/// The domains that get a cluster: each domain with a step, or batch for a graph with no step.
/// shortcut: one cluster for each domain, with no boundary at large nodes (8.2). Split at large nodes and subgraphs when milestone 6 measures the need.
pub fn domains(plan: &Plan) -> Vec<Domain> {
    let mut domains: Vec<Domain> =
        Domain::ALL.into_iter().filter(|&d| plan.steps.iter().any(|step| step.domain == d)).collect();
    if domains.is_empty() {
        domains.push(Domain::Batch);
    }
    domains
}

/// The package name of the cluster of `domain`. Its folder is `build/clusters/{domain name}`.
pub fn package(domain: Domain) -> String {
    format!("cluster-{}", domain.name())
}

/// The Rust name of the library target of `package(domain)`.
pub fn target(domain: Domain) -> String {
    format!("cluster_{}", domain.name())
}

/// The values that a cluster gives to the cluster of another domain (8.2), as a step index and an
/// output index. The index of a link is its position in the list.
pub fn links(plan: &Plan) -> Vec<(usize, usize)> {
    plan.held().filter(|&(n, k)| is_kept(plan, n, k)).collect()
}

/// True when a step of `domain` reads output `k` of step `n`.
pub fn reads(plan: &Plan, n: usize, k: usize, domain: Domain) -> bool {
    let wire = |s: &Source| matches!(s, Source::Wire { step, output } if (*step, *output) == (n, k));
    plan.steps.iter().any(|step| step.domain == domain && step.inputs.iter().any(wire))
}

/// The links that the cluster of `domain` reads from another cluster.
fn imports(plan: &Plan, domain: Domain) -> Vec<(usize, usize)> {
    links(plan).into_iter().filter(|&(n, k)| plan.steps[n].domain != domain && reads(plan, n, k, domain)).collect()
}

/// The steps that the witness of `domain` calls, in run order: the steps of the domain, the steps
/// before its imports, and the readers that give a type parameter of one of them (4.5).
fn witnessed(plan: &Plan, domain: Domain) -> BTreeSet<usize> {
    let own = (0..plan.steps.len()).filter(|&n| plan.steps[n].domain == domain);
    witnessed_from(plan, own.chain(imports(plan, domain).into_iter().map(|(n, _)| n)).collect())
}

/// The steps `todo`, the steps before them, and the readers that give a type parameter (4.5).
fn witnessed_from(plan: &Plan, mut todo: Vec<usize>) -> BTreeSet<usize> {
    let mut steps = BTreeSet::new();
    while let Some(n) = todo.pop() {
        if steps.insert(n) {
            todo.extend(plan.steps[n].inputs.iter().filter_map(|source| match source {
                Source::Wire { step, .. } => Some(*step),
                Source::Slot(_) => None,
            }));
            todo.extend(&plan.steps[n].pins);
        }
    }
    steps
}

pub fn manifest(plan: &Plan, domain: Domain) -> String {
    let package = package(domain);
    let mut text = format!(
        "{HEADER}[package]\nname = \"{package}\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\
         publish = false\n\n[lib]\ncrate-type = [\"cdylib\"]\npath = \"lib.rs\"\n\n\
         [dependencies]\npatina-sdk = {{ workspace = true }}\n"
    );
    // The witness calls steps of other domains too, so it needs their crates.
    let steps = witnessed(plan, domain);
    text.push_str(&crates(plan, &steps));
    for (name, _) in &plan.kinds {
        let _ = writeln!(text, "{name} = {{ path = \"../../../connections/{name}\" }}");
    }
    if plan.log {
        text.push_str("patina-log = { package = \"log\", version = \"0.4\" }\n");
    }
    if domain == Domain::Event || steps.iter().any(|&n| plan.nodes[plan.steps[n].node].1.is_async) {
        text.push_str("patina-event = { workspace = true }\n");
    }
    text
}

/// The dependency lines of the node crates of `steps`.
fn crates(plan: &Plan, steps: &BTreeSet<usize>) -> String {
    let mut names: Vec<&str> =
        steps.iter().map(|&n| plan.steps[n].node).filter(|&i| !plan.is_builtin_node(i)).map(|i| plan.nodes[i].0.as_str()).collect();
    names.sort_unstable();
    names.dedup();
    names.iter().map(|name| format!("{name} = {{ path = \"../../../nodes/{name}\" }}\n")).collect()
}

pub fn source(plan: &Plan, domain: Domain) -> String {
    let mut s = String::from(
        "// Made by Patina. Patina writes this file again at each build.\n#![allow(unused)]\n\n\
         use ::patina_sdk::glue::{DebugText as _, NameText as _, ViewerText as _};\n\
         use ::patina_sdk::glue::{NotSyncCheck as _, SyncCheck as _};\n\
         use ::patina_sdk::glue::{CloneKeep as _, NoKeep as _};\n\n\
         fn make() -> ::patina_sdk::glue::Cluster {\n",
    );
    if plan.log {
        // A second cluster of the same library gets an error here, and the first logger stays.
        s.push_str("    let _ = ::patina_log::set_logger(&PatinaLog);\n");
        s.push_str("    ::patina_log::set_max_level(::patina_log::LevelFilter::Info);\n");
    }
    let mut shared = Vec::new();
    let mine = |n: usize| plan.steps[n].domain == domain;
    // The engine reads the settings of a trigger (9.4), so they get no value in the cluster.
    let own = |slot: &&crate::plan::Slot| mine(slot.step) && !plan.is_trigger(slot.step);
    for (i, slot) in plan.slots.iter().enumerate().filter(|(_, slot)| own(slot)) {
        let ty = if slot.is_str { ": ::patina_sdk::glue::Shared<::std::string::String>" } else { "" };
        let step = &plan.steps[slot.step];
        let word = plan.nodes[step.node].1.inputs.iter().any(|i| i.key == slot.key && crate::plan::is_word(&i.value));
        let cell = if word { "Atom" } else { "Shared" };
        let _ = writeln!(
            s,
            "    let p{i}{ty} = ::patina_sdk::glue::{cell}::new(::patina_sdk::glue::default_value()); // {}.{}",
            step.id, slot.key
        );
        shared.push(format!("p{i}"));
    }
    witness(&mut s, plan, witnessed(plan, domain));
    // A value that another domain reads is in an `Arc`, which the other cluster links to (8.2).
    // A wire into or out of the audio domain is a buffer without a lock (9.3).
    for (n, k) in plan.held().filter(|&(n, k)| is_shared(plan, n, k)) {
        let empty = format!(
            "if false {{ ::std::option::Option::Some(::patina_sdk::glue::like(&w{n}).{k}) }} else {{ ::std::option::Option::None }}"
        );
        let value = if !mine(n) {
            if !reads(plan, n, k, domain) {
                continue;
            }
            if is_latest(plan, n, k) { "::patina_sdk::glue::LatestLink::new()".to_owned() } else { "::patina_sdk::glue::Link::new()".to_owned() }
        } else if is_latest(plan, n, k) {
            "::patina_sdk::glue::Latest::new()".to_owned()
        } else if is_kept(plan, n, k) {
            format!("::std::sync::Arc::new(::patina_sdk::glue::Shared::new({empty}))")
        } else if is_atom(plan, n, k) {
            format!("::patina_sdk::glue::Atom::new({empty})")
        } else {
            format!("::patina_sdk::glue::Shared::new({empty})")
        };
        let _ = writeln!(s, "    let n{n}_{k} = {value};");
        if !mine(n) {
            let _ = writeln!(s, "    if false {{ n{n}_{k}.witness(::patina_sdk::glue::like(&w{n}).{k}); }}");
        }
        shared.push(format!("n{n}_{k}"));
    }
    let slotted = (0..plan.steps.len()).any(|n| mine(n) && plan.in_slot(n));
    if slotted {
        s.push_str("    let slot = ::patina_sdk::glue::Slot::new();\n");
        shared.push("slot".to_owned());
    }
    // Each use of a stateful node has its own state (4.2). It drops with the cluster. The slot
    // library keeps the states of the node in the slot (7.1).
    let stateful: Vec<usize> = plan.stateful().filter(|&n| mine(n) && !plan.in_slot(n)).collect();
    for &n in &stateful {
        let _ = writeln!(s, "    let s{n} = ::patina_sdk::glue::State::new(); // {}", plan.steps[n].id);
        shared.push(format!("s{n}"));
    }
    // The value of each delay for the next frame (9.2). The first frame gives `Default::default()`.
    for (n, step) in plan.steps.iter().enumerate().filter(|&(n, _)| plan.is_delay(n) && mine(n)) {
        let Source::Wire { step: from, output } = step.inputs[0] else { continue };
        let _ = writeln!(s, "    let d{n} = ::patina_sdk::glue::delay_for(&n{from}_{output}); // {}", step.id);
        shared.push(format!("d{n}"));
    }
    // Rust knows these types only after the closure, so the checks for `parallel` start here.
    for name in &shared {
        let _ = writeln!(s, "    let c_{name} = ::patina_sdk::glue::check(&{name});");
    }
    let held: usize = plan.steps.iter().map(|step| step.consumers.iter().filter(|&&c| c > 0).count()).sum();
    let _ = writeln!(s, "    let keeps = ::patina_sdk::glue::Keeps::new({held});");
    s.push_str(
        "    let f = ::patina_sdk::glue::typed(move |cmd, rep| match cmd {\n        \
         ::patina_sdk::glue::Cmd::Set(index, text) => match index {\n",
    );
    for (i, slot) in plan.slots.iter().enumerate().filter(|(_, slot)| own(slot)) {
        let _ = writeln!(s, "            {i} => ::patina_sdk::glue::set_param(&mut *p{i}.write(), text, rep, {}),", slot.step);
    }
    s.push_str("            _ => false,\n        },\n");
    let mut probes = Vec::with_capacity(plan.steps.len());
    let mut probe = 0;
    for step in &plan.steps {
        probes.push(probe);
        probe += step.consumers.iter().filter(|&&c| c > 0).count();
    }
    let gates: Vec<bool> = (0..plan.steps.len()).map(|n| is_gate(plan, n)).collect();
    // The tasks with a delay come last, so Rust knows the type of the delay before its probe (9.2).
    let mut order: Vec<usize> = (0..plan.tasks.len()).filter(|&t| plan.tasks[t].domain == domain).collect();
    order.sort_by_key(|&t| plan.tasks[t].steps.iter().any(|&n| plan.is_delay(n)));
    for t in order {
        let task = &plan.tasks[t];
        // An event task runs once for each event of its trigger, with the text of the event (9.4).
        let cmd = if task.domain == Domain::Event { format!("Fire({t}, event)") } else { format!("Task({t})") };
        let _ = writeln!(s, "        ::patina_sdk::glue::Cmd::{cmd} => {{\n            let mut ok = true;");
        let local: Vec<(usize, usize)> =
            plan.held().filter(|&(n, k)| plan.steps[n].task == t && !is_shared(plan, n, k)).collect();
        // The witness gives the type, because a probe can come before the reader that gives it (4.5).
        for (n, k) in &local {
            let _ = writeln!(
                s,
                "            let mut n{n}_{k} = if false {{ ::std::option::Option::Some(::patina_sdk::glue::like(&w{n}).{k}) }} else {{ ::std::option::Option::None }};"
            );
        }
        for run in task.steps.chunk_by(|&a, &b| gates[a] && gates[b]) {
            if gates[run[0]] {
                net(&mut s, plan, run, &probes);
            } else {
                step(&mut s, plan, run[0], probes[run[0]]);
            }
        }
        for (n, k) in &local {
            let _ = writeln!(s, "            ok &= ::patina_sdk::glue::drop_value(n{n}_{k}, rep, {n});");
        }
        s.push_str("            ok\n        }\n");
    }
    s.push_str("        ::patina_sdk::glue::Cmd::Task(_) => false,\n        ::patina_sdk::glue::Cmd::Fire(..) => false,\n");
    // The GUI of each node with a `ui` function (4.3). The CLI never calls it.
    s.push_str("        ::patina_sdk::glue::Cmd::Ui(index, mode, events) => match index {\n");
    ui_arms(&mut s, plan, &stateful);
    s.push_str("            _ => false,\n        },\n");
    s.push_str(if slotted { "        ::patina_sdk::glue::Cmd::Slot(p) => slot.set(p),\n" } else { "        ::patina_sdk::glue::Cmd::Slot(_) => true,\n" });
    s.push_str("        ::patina_sdk::glue::Cmd::Call(..) => false,\n");
    s.push_str("        ::patina_sdk::glue::Cmd::Read(index, out) => { *out = keeps.read(index); true }\n");
    // The states move from an old cluster to a new cluster when their keys did not change (7.2).
    s.push_str("        ::patina_sdk::glue::Cmd::Take(index, out) => match index {\n");
    for n in &stateful {
        let _ = writeln!(s, "            {n} => ::patina_sdk::glue::take_state(&s{n}, out),");
    }
    s.push_str("            _ => false,\n        },\n        ::patina_sdk::glue::Cmd::Put(index, moved) => match index {\n");
    for n in &stateful {
        let _ = writeln!(s, "            {n} => ::patina_sdk::glue::put_state(&s{n}, moved, rep, {n}),");
    }
    s.push_str("            _ => false,\n        },\n");
    // The engine links each value that another domain reads after it makes the clusters (8.2).
    s.push_str("        ::patina_sdk::glue::Cmd::Export(index, out) => match index {\n");
    for (i, &(n, k)) in links(plan).iter().enumerate().filter(|(_, (n, _))| mine(*n)) {
        let f = if is_latest(plan, n, k) { "export_latest" } else { "export_link" };
        let _ = writeln!(s, "            {i} => ::patina_sdk::glue::{f}(&n{n}_{k}, out),");
    }
    s.push_str("            _ => false,\n        },\n        ::patina_sdk::glue::Cmd::Import(index, moved) => match index {\n");
    let imports = imports(plan, domain);
    for (i, &(n, k)) in links(plan).iter().enumerate().filter(|(_, link)| imports.contains(link)) {
        let f = if is_latest(plan, n, k) { "import_latest" } else { "import_link" };
        let _ = writeln!(s, "            {i} => ::patina_sdk::glue::{f}(&n{n}_{k}, moved),");
    }
    s.push_str("            _ => false,\n        },\n");
    // The start of a run gets the newest value of each wire from the audio domain or into it.
    let _ = write!(s, "        ::patina_sdk::glue::Cmd::Begin({}) => {{ ", domain as u32);
    for (n, k) in imports.iter().filter(|&&(n, k)| is_latest(plan, n, k)) {
        let _ = write!(s, "n{n}_{k}.fetch(); ");
    }
    s.push_str("true }\n        ::patina_sdk::glue::Cmd::Begin(_) => true,\n");
    // The end of a domain drops the values of its steps (9.1). A value that another domain reads
    // stays until its step runs again.
    let _ = writeln!(s, "        ::patina_sdk::glue::Cmd::End({}) => {{\n            let mut ok = true;", domain as u32);
    // Each delay keeps its input for the next frame, before the values drop (9.2).
    for (n, step) in plan.steps.iter().enumerate().filter(|&(n, step)| plan.is_delay(n) && step.domain == domain) {
        let Source::Wire { step: from, output } = step.inputs[0] else { continue };
        let value = if plan.steps[from].consumers[output] == 1 && plan.steps[from].domain == domain {
            format!("n{from}_{output}.write().take()")
        } else {
            format!("::std::clone::Clone::clone(&*n{from}_{output}.read())")
        };
        let _ = writeln!(s, "            if let ::std::option::Option::Some(v) = {value} {{ *d{n}.write() = v; }} // {}", step.id);
    }
    for (n, k) in plan.held().filter(|&(n, k)| plan.steps[n].domain == domain && is_shared(plan, n, k) && !is_kept(plan, n, k)) {
        let _ = writeln!(s, "            ok &= ::patina_sdk::glue::drop_value(n{n}_{k}.write().take(), rep, {n});");
    }
    s.push_str("            ok\n        }\n");
    s.push_str("        ::patina_sdk::glue::Cmd::End(_) => true,\n    });\n    let parallel = true");
    for name in &shared {
        let _ = write!(s, " && (&&c_{name}).sync()");
    }
    s.push_str(";\n    ::patina_sdk::glue::cluster(f, parallel)\n}\n");
    s.push_str(EXPORTS);
    if plan.log {
        s.push_str(LOGGER);
    }
    s
}

/// Writes the `Ui` arm of each step of `steps` with a `ui` function (4.3).
fn ui_arms(s: &mut String, plan: &Plan, steps: &[usize]) {
    for &n in steps {
        let (name, node) = &plan.nodes[plan.steps[n].node];
        let (Some(state), true) = (&node.state, node.ui) else { continue };
        let krate = name.replace('-', "_");
        let _ = writeln!(
            s,
            "            {n} => ::patina_sdk::glue::draw(&s{n}, ::{krate}::{state}::init, |st, ui| st.ui(ui), (mode, events), rep, {n}),"
        );
    }
}

/// Writes a witness `w{m}` for each step of `steps`: `None` at
/// run time, with the type of the tuple of the step outputs. Each value then gets its type where
/// it is made, and Rust does not carry thousands of open types through the closure (8.2). A slot
/// gives its default. A delay runs before the step of its input, so it gets that type at the end.
fn witness(s: &mut String, plan: &Plan, steps: BTreeSet<usize>) {
    let mut ties = String::new();
    for m in steps {
        let step = &plan.steps[m];
        if plan.is_trigger(m) {
            let ty = if plan.builtin(m) == Some(Builtin::Schedule) { "0u64" } else { "::std::string::String::new()" };
            let _ = writeln!(s, "    let w{m} = if false {{ ::std::option::Option::Some(({ty},)) }} else {{ ::std::option::Option::None }}; // {}", step.id);
            continue;
        }
        if plan.is_delay(m) {
            if let Source::Wire { step: from, output } = step.inputs[0] {
                let _ = writeln!(
                    ties,
                    "    if false {{ ::patina_sdk::glue::same(&::patina_sdk::glue::like(&w{m}).0, &::patina_sdk::glue::like(&w{from}).{output}); }}"
                );
            }
            let _ = writeln!(s, "    let w{m}: ::std::option::Option<(_,)> = ::std::option::Option::None; // {}", step.id);
            continue;
        }
        let (name, node) = &plan.nodes[step.node];
        let krate = name.replace('-', "_");
        let (mut lets, mut args, mut call) = (String::new(), Vec::new(), format!("::{krate}::{}", node.func));
        if let Some(state) = &node.state {
            args.push(format!("&mut ::{krate}::{state}::init()"));
            call = format!("::{krate}::{state}::{}", node.func);
        }
        for (input, source) in node.inputs.iter().zip(&step.inputs) {
            let owned = match source {
                Source::Slot(i) if plan.slots[*i].is_str => "::patina_sdk::glue::default_value::<::std::string::String>()".to_owned(),
                Source::Slot(_) => "::patina_sdk::glue::default_value()".to_owned(),
                Source::Wire { step: from, output } => format!("::patina_sdk::glue::like(&w{from}).{output}"),
            };
            match input.pass {
                Pass::Ref => args.push(format!("&{owned}")),
                Pass::Take => args.push(owned),
                Pass::Mut => {
                    let _ = write!(lets, "let mut m_{} = {owned}; ", input.key);
                    args.push(format!("&mut m_{}", input.key));
                }
            }
        }
        let mut call = format!("{call}({})", args.join(", "));
        if node.is_async {
            call = format!("::patina_event::output({call})");
        }
        if node.fallible {
            call = format!("match {call} {{ ::std::result::Result::Ok(v) => v, ::std::result::Result::Err(_) => ::std::unreachable!() }}");
        }
        let (unpack, mut values) = match node.output {
            Output::None => ("let () = r; ".to_owned(), Vec::new()),
            Output::One => (String::new(), vec!["r".to_owned()]),
            Output::Tuple(count) => {
                let names: Vec<String> = (0..count).map(|i| format!("r{i}")).collect();
                (format!("let ({},) = r; ", names.join(", ")), names)
            }
        };
        values.extend(node.inputs.iter().filter(|i| i.pass == Pass::Mut).map(|i| format!("m_{}", i.key)));
        if values.is_empty() {
            // A reader with no output still gives the type parameters of its input step (4.5).
            if plan.steps.iter().any(|other| other.pins.contains(&m)) {
                let _ = writeln!(s, "    if false {{ {lets}let _ = {call}; }} // {}", step.id);
            }
            continue;
        }
        let _ = writeln!(
            s,
            "    let w{m} = if false {{ ::std::option::Option::Some({{ {lets}let r = {call}; {unpack}({},) }}) }} else {{ ::std::option::Option::None }}; // {}",
            values.join(", "),
            step.id
        );
    }
    s.push_str(&ties);
}

/// True when a node of another task or a delay reads output `k` of step `n`. The value is then in
/// a shared slot of the cluster. Else it is a local variable of its task (9.2).
fn is_shared(plan: &Plan, n: usize, k: usize) -> bool {
    let reads = |s: &Source| matches!(s, Source::Wire { step, output } if (*step, *output) == (n, k));
    plan.steps.iter().enumerate().any(|(m, step)| (step.task != plan.steps[n].task || plan.is_delay(m)) && step.inputs.iter().any(reads))
}

/// True when output `k` of step `n` is a number that goes to another task of its domain. It goes
/// in an `Atom`, without a lock (9.2). A delay reads a `Shared` value.
fn is_atom(plan: &Plan, n: usize, k: usize) -> bool {
    let reads = |s: &Source| matches!(s, Source::Wire { step, output } if (*step, *output) == (n, k));
    crate::plan::is_word(&plan.steps[n].types[k])
        && !is_kept(plan, n, k)
        && !plan.steps.iter().enumerate().any(|(m, step)| plan.is_delay(m) && step.inputs.iter().any(reads))
}

/// True when a node of another domain reads output `k` of step `n`: a latest-value wire (9.1).
/// Batch ends before the frames start, so its lock never waits.
fn is_kept(plan: &Plan, n: usize, k: usize) -> bool {
    let reads = |s: &Source| matches!(s, Source::Wire { step, output } if (*step, *output) == (n, k));
    plan.steps.iter().any(|step| step.domain != plan.steps[n].domain && step.inputs.iter().any(reads))
}

/// True for a latest-value wire into or out of the audio domain. It does not lock (9.3).
fn is_latest(plan: &Plan, n: usize, k: usize) -> bool {
    is_kept(plan, n, k) && (plan.steps[n].domain == Domain::Audio || reads(plan, n, k, Domain::Audio))
}

/// The read, the write and the store expressions of output `k` of step `n`.
fn held(plan: &Plan, n: usize, k: usize) -> [String; 3] {
    let name = format!("n{n}_{k}");
    if is_shared(plan, n, k) {
        [format!("{name}.read()"), format!("{name}.write()"), format!("*{name}.write()")]
    } else {
        [name.clone(), name.clone(), name]
    }
}

/// Writes the call of one node. It runs only when each wired input has a value (15.3) and the
/// user did not click **Stop** (15.2). Else `enter` reports why. After the call, it sends the text
/// and the lanes (6.3) of each held value, before a later node takes the value (15.9). `probe` is
/// the index of the first held value of the step.
fn step(s: &mut String, plan: &Plan, n: usize, probe: usize) {
    let step = &plan.steps[n];
    let name = &plan.nodes[step.node].0;
    // A delay gives the value of the previous frame, so it waits for no input (9.2).
    let delay = plan.is_delay(n);
    let wired: Vec<String> = step
        .inputs
        .iter()
        .filter(|_| !delay)
        .filter_map(|source| match source {
            Source::Wire { step, output } => Some(format!("{}.is_some()", held(plan, *step, *output)[0])),
            Source::Slot(_) => None,
        })
        .collect();
    let ready = if wired.is_empty() { "true".to_owned() } else { wired.join(" && ") };
    let kind = match plan.builtin(n) {
        Some(builtin) => format!("{} (9.2, 9.4)", builtin.name()),
        None => format!("nodes/{name}"),
    };
    let _ = writeln!(s, "            // {}: {kind}", step.id);
    for k in (0..step.consumers.len()).filter(|&k| is_kept(plan, n, k)) {
        let _ = writeln!(s, "            ok &= ::patina_sdk::glue::drop_value(n{n}_{k}.write().take(), rep, {n});");
    }
    let _ = writeln!(s, "            if rep.enter({n}, {ready}) {{");
    if plan.in_slot(n) {
        slot::call(s, plan, n);
    } else if plan.is_trigger(n) {
        fire(s, plan, n);
    } else {
        inline(s, plan, n);
    }
    probes(s, plan, n, probe);
    s.push_str("            }\n");
    for k in (0..step.consumers.len()).filter(|&k| is_latest(plan, n, k)) {
        let _ = writeln!(s, "            n{n}_{k}.publish();");
    }
}

/// True when step `n` can be a gate of a net (9.2): a light node with no state, no error, no
/// `&mut` input and one output, which no other domain reads.
fn is_gate(plan: &Plan, n: usize) -> bool {
    let node = &plan.nodes[plan.steps[n].node].1;
    plan.is_light(n)
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
/// inputs, and the probes of the net come after the block (15.9). After a panic, the block runs
/// again from the next gate, so each gate gets the result of its own catch (4.7, 15.3).
fn net(s: &mut String, plan: &Plan, net: &[usize], first: &[usize]) {
    let nodes: Vec<String> = net.iter().map(usize::to_string).collect();
    let _ = writeln!(s, "            if let ::std::option::Option::Some(net) = ::patina_sdk::glue::Net::new(rep, &[{}]) {{", nodes.join(", "));
    s.push_str("                ok &= net.run(&mut |at, cur| {
");
    for (i, &n) in net.iter().enumerate() {
        let step = &plan.steps[n];
        let (name, node) = &plan.nodes[step.node];
        let (mut pats, mut wired, mut args) = (Vec::new(), Vec::new(), Vec::new());
        for (input, source) in node.inputs.iter().zip(&step.inputs) {
            let value = match *source {
                Source::Slot(p) => format!("*p{p}.read()"),
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
            format!("net.enter({n}); {} = ::std::option::Option::Some({call});", held(plan, n, 0)[2])
        } else {
            format!("net.enter({n}); let _ = {call};")
        };
        let run = if wired.is_empty() {
            run
        } else {
            format!("if let ({},) = ({},) {{ {run} }} else {{ net.skip({n}); }}", pats.join(", "), wired.join(", "))
        };
        let at = if i == 0 { "at == 0".to_owned() } else { format!("at <= {i}") };
        let _ = writeln!(s, "                    if {at} {{ *cur = {i}; {run} }} // {}: nodes/{name}", step.id);
    }
    s.push_str("                });\n");
    // A gate has one output. One check of `quiet` and one of `any` cover the probes of the net.
    let shown: Vec<usize> = net.iter().copied().filter(|&n| plan.steps[n].consumers[0] > 0).collect();
    if !shown.is_empty() {
        s.push_str("                if !rep.quiet() {\n");
        for &n in &shown {
            let _ = writeln!(s, "                    if let ::std::option::Option::Some(v) = {}.as_ref() {{ {} }}", held(plan, n, 0)[0], send(plan, n, 0, first[n]));
        }
        s.push_str("                }\n                if keeps.any() {\n");
        for &n in &shown {
            let _ = writeln!(s, "                    if let ::std::option::Option::Some(v) = {}.as_ref() {{ {} }}", held(plan, n, 0)[0], keep(first[n]));
        }
        s.push_str("                }\n");
    }
    s.push_str("            }
");
}

/// Stores the event of trigger step `n` (9.4): the text of a trigger, or the count of a schedule.
fn fire(s: &mut String, plan: &Plan, n: usize) {
    if plan.steps[n].consumers[0] > 0 {
        let value = if plan.builtin(n) == Some(Builtin::Schedule) { "event.parse::<u64>().unwrap_or(0)" } else { "::std::borrow::ToOwned::to_owned(event)" };
        let _ = writeln!(s, "                {} = ::std::option::Option::Some({value});", held(plan, n, 0)[2]);
    }
}

/// Writes the call of step `n` in the cluster. `glue::run` gives the call its own catch (4.7), so
/// the code has no closure for each node, and a large cluster builds fast.
fn inline(s: &mut String, plan: &Plan, n: usize) {
    let step = &plan.steps[n];
    let (name, node) = &plan.nodes[step.node];
    let indent = "                ";
    let krate = name.replace('-', "_");
    let (mut args, mut muts) = (Vec::new(), Vec::new());
    let mut call = format!("::{krate}::{}", node.func);
    let plain = node.state.is_none() && !plan.is_delay(n) && !node.is_async && !node.fallible && node.output == Output::One;
    if plain && step.consumers[0] > 0 && node.inputs.iter().all(|i| i.pass != Pass::Mut) {
        let args: String = inputs(plan, n, true).iter().map(|arg| format!("{arg}, ")).collect();
        let _ = writeln!(s, "{indent}ok &= ::patina_sdk::glue::step(rep, {n}, {call}, ({args}), &mut {});", held(plan, n, 0)[2]);
        return;
    }
    if let Some(state) = &node.state {
        let _ = writeln!(s, "{indent}let mut st = s{n}.lock();");
        // A panic in `init` leaves no state, so the next run calls `init` again (4.7).
        args.push(format!("::patina_sdk::glue::Init(&mut *st, ::{krate}::{state}::init)"));
        call = format!("::{krate}::{state}::{}", node.func);
    }
    let _ = writeln!(s, "{indent}let t = ::std::time::Instant::now();");
    if plan.is_delay(n) {
        call = "::std::mem::take".to_owned();
        args.push(format!("&mut *d{n}.write()"));
    } else {
        for (input, value) in node.inputs.iter().zip(inputs(plan, n, true)) {
            if input.pass == Pass::Mut {
                let _ = writeln!(s, "{indent}let mut m_{} = ::std::option::Option::None;", input.key);
                args.push(format!("::patina_sdk::glue::Hold(&mut m_{}, {value})", input.key));
                muts.push(&input.key);
            } else {
                args.push(value);
            }
        }
    }
    let args: String = args.iter().map(|arg| format!("{arg}, ")).collect();
    let run = if node.fallible { "try_run" } else { "run" };
    if node.is_async {
        // The run waits for the future of an async node, and stops it at **Stop** (9.4).
        let _ = writeln!(s, "{indent}let r = ::patina_event::{run}({call}, ({args}), rep, {n});");
        let _ = writeln!(s, "{indent}if let ::std::option::Option::Some(r) = ::patina_event::done(r, rep, {n}, &mut ok) {{");
    } else {
        let _ = writeln!(s, "{indent}let r = ::patina_sdk::glue::{run}({call}, ({args}));");
        let done = if node.state.is_some() { "done_state(r, &mut *st, " } else { "done(r, " };
        let _ = writeln!(s, "{indent}if let ::std::option::Option::Some(r) = ::patina_sdk::glue::{done}rep, {n}, &mut ok) {{");
    }
    let inner = format!("{indent}    ");
    for key in &muts {
        // A call that worked gave the value to each `Hold`.
        let _ = writeln!(s, "{inner}let m_{key} = m_{key}.unwrap();");
    }
    store(s, plan, n, &inner);
    s.push_str(indent);
    s.push('}');
    if !muts.is_empty() {
        s.push_str(" else {");
        for key in &muts {
            let _ = write!(s, " ok &= ::patina_sdk::glue::drop_value(m_{key}, rep, {n});");
        }
        s.push_str(" }");
    }
    s.push('\n');
    let _ = writeln!(s, "{indent}rep.time({n}, t);");
}

/// The expression of the value of each input of step `n`, in the order of the node inputs. With
/// `wrap`, each copy and each moved value is a `glue::Arg`, so the copy runs in the catch.
fn inputs(plan: &Plan, n: usize, wrap: bool) -> Vec<String> {
    let step = &plan.steps[n];
    let node = &plan.nodes[step.node].1;
    let cloned = |value: String| if wrap { format!("::patina_sdk::glue::Cloned({value})") } else { format!("::std::clone::Clone::clone({value})") };
    let moved = |value: String| if wrap { format!("::patina_sdk::glue::Take({value})") } else { value };
    let value = |(input, source): (&crate::node::Input, &Source)| match (source, input.pass) {
        // A call through `glue::run` has no deref coercion, so the slot `String` becomes a `&str`.
        (Source::Slot(i), Pass::Ref) if input.is_str && wrap => format!("::std::string::String::as_str(&*p{i}.read())"),
        (Source::Slot(i), Pass::Ref) => format!("&*p{i}.read()"),
        (Source::Slot(i), _) => cloned(format!("&*p{i}.read()")),
        (Source::Wire { step: from, output }, pass) => {
            let [read, write, _] = held(plan, *from, *output);
            match pass {
                Pass::Ref => format!("{read}.as_ref().unwrap()"),
                // Only one node reads the value, so it takes the value with no copy. A frame
                // reads a batch value again at each frame.
                _ if plan.steps[*from].consumers[*output] == 1 && plan.steps[*from].domain == step.domain => {
                    moved(format!("{write}.take().unwrap()"))
                }
                _ => cloned(format!("{read}.as_ref().unwrap()")),
            }
        }
    };
    node.inputs.iter().zip(&step.inputs).map(value).collect()
}

/// Unpacks the result `r` of step `n`, and stores each output and each `m_` value that a node
/// reads. It drops the other values, each with its own catch.
fn store(s: &mut String, plan: &Plan, n: usize, indent: &str) {
    let node = &plan.nodes[plan.steps[n].node].1;
    let mut values: Vec<String> = match node.output {
        Output::None => {
            let _ = writeln!(s, "{indent}let () = r;");
            Vec::new()
        }
        Output::One => vec!["r".to_owned()],
        Output::Tuple(count) => {
            let names: Vec<String> = (0..count).map(|i| format!("r{i}")).collect();
            let _ = writeln!(s, "{indent}let ({},) = r;", names.join(", "));
            names
        }
    };
    values.extend(node.inputs.iter().filter(|i| i.pass == Pass::Mut).map(|i| format!("m_{}", i.key)));
    for (k, value) in values.iter().enumerate() {
        if plan.steps[n].consumers[k] > 0 {
            let _ = writeln!(s, "{indent}{} = ::std::option::Option::Some({value});", held(plan, n, k)[2]);
        } else {
            let _ = writeln!(s, "{indent}ok &= ::patina_sdk::glue::drop_value(::std::option::Option::Some({value}), rep, {n});");
        }
    }
}

/// Writes the probes of step `n` (15.9). `probe` is the index of its first held value.
fn probes(s: &mut String, plan: &Plan, n: usize, mut probe: usize) {
    let step = &plan.steps[n];
    let indent = "                ";
    for k in (0..step.consumers.len()).filter(|k| step.consumers[*k] > 0) {
        let _ = writeln!(
            s,
            "{indent}if let ::std::option::Option::Some(v) = {}.as_ref() {{ {} {} }}",
            held(plan, n, k)[0],
            send(plan, n, k, probe),
            keep(probe)
        );
        probe += 1;
    }
}

/// The code that sends the text and the lanes (6.3) of held value `v` with index `probe`.
fn send(plan: &Plan, n: usize, k: usize, probe: usize) -> String {
    let lanes = plan.kind(n, k).map_or(String::new(), |(name, kind)| {
        format!(" ::patina_sdk::glue::send_lanes(rep, {probe}, v, ::{}::{});", name.replace('-', "_"), kind.func)
    });
    format!("::patina_sdk::glue::send_value(rep, {probe}, v, (&&&::patina_sdk::glue::Probe(v)).text_fn());{lanes}")
}

/// The code that keeps a copy of held value `v` with index `probe` for a view (15.8).
fn keep(probe: usize) -> String {
    format!("(&&::patina_sdk::glue::Probe(v)).keep(&keeps, {probe});")
}

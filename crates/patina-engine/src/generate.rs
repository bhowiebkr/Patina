//! Writes the cluster crates of a plan (spec 8.2): one crate for each clock domain.
//!
//! The code names no type of a node crate: the parameters live in one closure, and Rust infers
//! each type from the node calls. The code uses only full paths, because a node crate can have
//! the name of any `use`.

use std::fmt::Write;

use crate::node::{Domain, Output, Pass};
use crate::plan::{Plan, Source};

const HEADER: &str = "# Made by Patina. Patina writes this file again at each build.\n";

/// The domains that get a cluster: each domain with a step, or batch for a graph with no step.
/// shortcut: one cluster for each domain, with no boundary at large nodes (8.2). Split at large nodes and subgraphs when milestone 6 measures the need.
pub fn domains(plan: &Plan) -> Vec<Domain> {
    let mut domains: Vec<Domain> =
        [Domain::Batch, Domain::Frame].into_iter().filter(|&d| plan.steps.iter().any(|step| step.domain == d)).collect();
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

/// The steps that make the imports of `domain`, and the steps before them, in run order.
fn ancestors(plan: &Plan, domain: Domain) -> std::collections::BTreeSet<usize> {
    let mut steps = std::collections::BTreeSet::new();
    let mut todo: Vec<usize> = imports(plan, domain).into_iter().map(|(n, _)| n).collect();
    while let Some(n) = todo.pop() {
        if steps.insert(n) {
            todo.extend(plan.steps[n].inputs.iter().filter_map(|source| match source {
                Source::Wire { step, .. } => Some(*step),
                Source::Slot(_) => None,
            }));
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
    // The witness of the links calls the steps before them, so it needs their crates too.
    let steps = (0..plan.steps.len()).filter(|&n| plan.steps[n].domain == domain).chain(ancestors(plan, domain));
    let mut names: Vec<&str> =
        steps.map(|n| plan.steps[n].node).filter(|&i| plan.delay != Some(i)).map(|i| plan.nodes[i].0.as_str()).collect();
    names.sort_unstable();
    names.dedup();
    for name in names {
        let _ = writeln!(text, "{name} = {{ path = \"../../../nodes/{name}\" }}");
    }
    for (name, _) in &plan.kinds {
        let _ = writeln!(text, "{name} = {{ path = \"../../../connections/{name}\" }}");
    }
    if plan.log {
        text.push_str("patina-log = { package = \"log\", version = \"0.4\" }\n");
    }
    text
}

pub fn source(plan: &Plan, domain: Domain) -> String {
    let mut s = String::from(
        "// Made by Patina. Patina writes this file again at each build.\n#![allow(unused)]\n\n\
         use ::patina_sdk::glue::{DebugText as _, NameText as _, ViewerText as _};\n\
         use ::patina_sdk::glue::{NotSyncCheck as _, SyncCheck as _};\n\n\
         fn make() -> ::patina_sdk::glue::Cluster {\n",
    );
    if plan.log {
        // A second cluster of the same library gets an error here, and the first logger stays.
        s.push_str("    let _ = ::patina_log::set_logger(&PatinaLog);\n");
        s.push_str("    ::patina_log::set_max_level(::patina_log::LevelFilter::Info);\n");
    }
    let mut shared = Vec::new();
    let mine = |n: usize| plan.steps[n].domain == domain;
    for (i, slot) in plan.slots.iter().enumerate().filter(|(_, slot)| mine(slot.step)) {
        let ty = if slot.is_str { ": ::patina_sdk::glue::Shared<::std::string::String>" } else { "" };
        let id = &plan.steps[slot.step].id;
        let _ = writeln!(
            s,
            "    let p{i}{ty} = ::patina_sdk::glue::Shared::new(::patina_sdk::glue::default_value()); // {id}.{}",
            slot.key
        );
        shared.push(format!("p{i}"));
    }
    // A value that another domain reads is in an `Arc`, which the other cluster links to (8.2).
    for (n, k) in plan.held().filter(|&(n, k)| is_shared(plan, n, k)) {
        let value = if !mine(n) {
            if !reads(plan, n, k, domain) {
                continue;
            }
            "::patina_sdk::glue::Link::new()"
        } else if is_kept(plan, n, k) {
            "::std::sync::Arc::new(::patina_sdk::glue::Shared::new(::std::option::Option::None))"
        } else {
            "::patina_sdk::glue::Shared::new(::std::option::Option::None)"
        };
        let _ = writeln!(s, "    let n{n}_{k} = {value};");
        shared.push(format!("n{n}_{k}"));
    }
    witness(&mut s, plan, domain);
    // Each use of a stateful node has its own state (4.2). It drops with the cluster.
    let stateful: Vec<usize> = plan.stateful().filter(|&n| mine(n)).collect();
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
    s.push_str(
        "    let f = ::patina_sdk::glue::typed(move |cmd, rep| match cmd {\n        \
         ::patina_sdk::glue::Cmd::Set(index, text) => match index {\n",
    );
    for (i, slot) in plan.slots.iter().enumerate().filter(|(_, slot)| mine(slot.step)) {
        let _ = writeln!(s, "            {i} => ::patina_sdk::glue::set_param(&mut *p{i}.write(), text, rep, {}),", slot.step);
    }
    s.push_str("            _ => false,\n        },\n");
    let mut probes = Vec::with_capacity(plan.steps.len());
    let mut probe = 0;
    for step in &plan.steps {
        probes.push(probe);
        probe += step.consumers.iter().filter(|&&c| c > 0).count();
    }
    // The tasks with a delay come last, so Rust knows the type of the delay before its probe (9.2).
    let mut order: Vec<usize> = (0..plan.tasks.len()).filter(|&t| plan.tasks[t].domain == domain).collect();
    order.sort_by_key(|&t| plan.tasks[t].steps.iter().any(|&n| plan.is_delay(n)));
    for t in order {
        let task = &plan.tasks[t];
        let _ = writeln!(s, "        ::patina_sdk::glue::Cmd::Task({t}) => {{\n            let mut ok = true;");
        let local: Vec<(usize, usize)> =
            plan.held().filter(|&(n, k)| plan.steps[n].task == t && !is_shared(plan, n, k)).collect();
        for (n, k) in &local {
            let _ = writeln!(s, "            let mut n{n}_{k} = ::std::option::Option::None;");
        }
        for &n in &task.steps {
            step(&mut s, plan, n, probes[n]);
        }
        for (n, k) in &local {
            let _ = writeln!(s, "            ok &= ::patina_sdk::glue::drop_value(n{n}_{k}, rep, {n});");
        }
        s.push_str("            ok\n        }\n");
    }
    s.push_str("        ::patina_sdk::glue::Cmd::Task(_) => false,\n");
    // The GUI of each node with a `ui` function (4.3). The CLI never calls it.
    s.push_str("        ::patina_sdk::glue::Cmd::Ui(index, mode, events) => match index {\n");
    for (n, step) in plan.steps.iter().enumerate().filter(|&(n, _)| mine(n)) {
        let (name, node) = &plan.nodes[step.node];
        let (Some(state), true) = (&node.state, node.ui) else { continue };
        let krate = name.replace('-', "_");
        let _ = writeln!(
            s,
            "            {n} => ::patina_sdk::glue::draw(&s{n}, ::{krate}::{state}::init, |st, ui| st.ui(ui), (mode, events), rep, {n}),"
        );
    }
    s.push_str("            _ => false,\n        },\n");
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
        let _ = writeln!(s, "            {i} => ::patina_sdk::glue::export_link(&n{n}_{k}, out),");
    }
    s.push_str("            _ => false,\n        },\n        ::patina_sdk::glue::Cmd::Import(index, moved) => match index {\n");
    let imports = imports(plan, domain);
    for (i, link) in links(plan).iter().enumerate().filter(|(_, link)| imports.contains(link)) {
        let _ = writeln!(s, "            {i} => ::patina_sdk::glue::import_link(&n{}_{}, moved),", link.0, link.1);
    }
    s.push_str("            _ => false,\n        },\n");
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

/// Writes a block that never runs. It calls each step before the imports of `domain`, so Rust
/// gives each link the type of the value of the other cluster (8.2). A slot gives its default.
/// The steps before an import are in the batch domain, which has no delay (9.2).
fn witness(s: &mut String, plan: &Plan, domain: Domain) {
    let imports = imports(plan, domain);
    if imports.is_empty() {
        return;
    }
    s.push_str("    if false {\n");
    for m in ancestors(plan, domain) {
        let step = &plan.steps[m];
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
                Source::Wire { step: from, output } => format!("::patina_sdk::glue::like(&w{from}_{output})"),
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
        let names: Vec<String> = (0..values.len()).map(|k| format!("w{m}_{k}")).collect();
        let _ = writeln!(
            s,
            "        let ({},) = {{ {lets}let r = {call}; {unpack}({},) }}; // {}",
            names.join(", "),
            values.join(", "),
            step.id
        );
    }
    for (n, k) in imports {
        let _ = writeln!(s, "        n{n}_{k}.witness(w{n}_{k});");
    }
    s.push_str("    }\n");
}

/// True when a node of another task or a delay reads output `k` of step `n`. The value is then in
/// a shared slot of the cluster. Else it is a local variable of its task (9.2).
fn is_shared(plan: &Plan, n: usize, k: usize) -> bool {
    let reads = |s: &Source| matches!(s, Source::Wire { step, output } if (*step, *output) == (n, k));
    plan.steps.iter().enumerate().any(|(m, step)| (step.task != plan.steps[n].task || plan.is_delay(m)) && step.inputs.iter().any(reads))
}

/// True when a node of another domain reads output `k` of step `n`: a latest-value wire (9.1).
/// shortcut: the domains never run at the same time, so the lock never waits. Use a buffer that does not lock with the audio domain (milestone 11).
fn is_kept(plan: &Plan, n: usize, k: usize) -> bool {
    let reads = |s: &Source| matches!(s, Source::Wire { step, output } if (*step, *output) == (n, k));
    plan.steps.iter().any(|step| step.domain != plan.steps[n].domain && step.inputs.iter().any(reads))
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
fn step(s: &mut String, plan: &Plan, n: usize, mut probe: usize) {
    let step = &plan.steps[n];
    let (name, node) = &plan.nodes[step.node];
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
    let _ = writeln!(s, "            // {}: {}", step.id, if delay { "delay (9.2)".to_owned() } else { format!("nodes/{name}") });
    for k in (0..step.consumers.len()).filter(|&k| is_kept(plan, n, k)) {
        let _ = writeln!(s, "            ok &= ::patina_sdk::glue::drop_value(n{n}_{k}.write().take(), rep, {n});");
    }
    let _ = writeln!(s, "            if rep.enter({n}, {ready}) {{");
    let indent = "                ";
    let krate = name.replace('-', "_");
    if node.state.is_some() {
        let _ = writeln!(s, "{indent}let mut st = s{n}.lock();");
    }
    let _ = writeln!(s, "{indent}let t = ::std::time::Instant::now();");
    let _ = writeln!(
        s,
        "{indent}ok &= ::patina_sdk::glue::{}(::std::panic::catch_unwind(\
         ::std::panic::AssertUnwindSafe(|| -> ::std::result::Result<(), ::std::string::String> {{",
        if node.state.is_some() { "finish_state" } else { "finish" }
    );
    let inner = format!("{indent}    ");
    let mut args = Vec::new();
    let mut call = format!("::{krate}::{}", node.func);
    if let Some(state) = &node.state {
        // A panic in `init` leaves no state, so the next run calls `init` again (4.7).
        let _ = writeln!(s, "{inner}let st = st.get_or_insert_with(::{krate}::{state}::init);");
        args.push("st".to_owned());
        call = format!("::{krate}::{state}::{}", node.func);
    }
    for (input, source) in node.inputs.iter().zip(&step.inputs).filter(|_| !delay) {
        let value = match (source, input.pass) {
            (Source::Slot(i), Pass::Ref) => format!("&*p{i}.read()"),
            (Source::Slot(i), _) => format!("::std::clone::Clone::clone(&*p{i}.read())"),
            (Source::Wire { step: from, output }, pass) => {
                let [read, write, _] = held(plan, *from, *output);
                match pass {
                    Pass::Ref => format!("{read}.as_ref().unwrap()"),
                    // Only one node reads the value, so it takes the value with no copy. A frame
                    // reads a batch value again at each frame.
                    _ if plan.steps[*from].consumers[*output] == 1 && plan.steps[*from].domain == step.domain => {
                        format!("{write}.take().unwrap()")
                    }
                    _ => format!("::std::clone::Clone::clone({read}.as_ref().unwrap())"),
                }
            }
        };
        if input.pass == Pass::Mut {
            let _ = writeln!(s, "{inner}let mut m_{} = {value};", input.key);
            args.push(format!("&mut m_{}", input.key));
        } else {
            args.push(value);
        }
    }
    let call = if delay { format!("::std::mem::take(&mut *d{n}.write())") } else { format!("{call}({})", args.join(", ")) };
    if node.fallible {
        let _ = writeln!(
            s,
            "{inner}let r = match {call} {{\n{inner}    ::std::result::Result::Ok(v) => v,\n\
             {inner}    ::std::result::Result::Err(e) => \
             return ::std::result::Result::Err(::std::format!(\"{{e:?}}\")),\n{inner}}};"
        );
    } else {
        let _ = writeln!(s, "{inner}let r = {call};");
    }
    let mut values: Vec<String> = match node.output {
        Output::None => {
            let _ = writeln!(s, "{inner}let () = r;");
            Vec::new()
        }
        Output::One => vec!["r".to_owned()],
        Output::Tuple(count) => {
            let names: Vec<String> = (0..count).map(|i| format!("r{i}")).collect();
            let _ = writeln!(s, "{inner}let ({},) = r;", names.join(", "));
            names
        }
    };
    values.extend(node.inputs.iter().filter(|i| i.pass == Pass::Mut).map(|i| format!("m_{}", i.key)));
    for (k, value) in values.iter().enumerate() {
        if step.consumers[k] > 0 {
            let _ = writeln!(s, "{inner}{} = ::std::option::Option::Some({value});", held(plan, n, k)[2]);
        } else {
            let _ = writeln!(s, "{inner}::std::mem::drop({value});");
        }
    }
    let state = if node.state.is_some() { "&mut *st, " } else { "" };
    let _ = writeln!(s, "{inner}::std::result::Result::Ok(())\n{indent}}})), {state}rep, {n});");
    let _ = writeln!(s, "{indent}rep.time({n}, t);");
    for k in (0..values.len()).filter(|k| step.consumers[*k] > 0) {
        let lanes = plan.kind(n, k).map_or(String::new(), |(name, kind)| {
            format!(" ::patina_sdk::glue::send_lanes(rep, {probe}, || ::{}::{}(v));", name.replace('-', "_"), kind.func)
        });
        let _ = writeln!(
            s,
            "{indent}if let ::std::option::Option::Some(v) = {}.as_ref() {{ ::patina_sdk::glue::send_value(rep, {probe}, \
             |out| (&&&::patina_sdk::glue::Probe(v)).text(out));{lanes} }}",
            held(plan, n, k)[0]
        );
        probe += 1;
    }
    s.push_str("            }\n");
}

const EXPORTS: &str = r#"
#[unsafe(no_mangle)]
pub extern "C" fn patina_abi_version() -> u32 {
    ::patina_sdk::glue::ABI_VERSION
}

#[unsafe(no_mangle)]
pub extern "C" fn patina_cluster_new() -> *mut ::std::ffi::c_void {
    ::patina_sdk::glue::new_cluster(make).cast()
}

/// # Safety
///
/// The engine keeps the rules of `patina_sdk::glue::set`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn patina_cluster_set(
    cluster: *mut ::std::ffi::c_void,
    index: u32,
    text: *const u8,
    len: usize,
    rep: *const ::patina_sdk::glue::Reporter,
) -> bool {
    // SAFETY: the engine keeps the rules of `glue::set`.
    unsafe { ::patina_sdk::glue::set(cluster.cast(), index, text, len, rep) }
}

/// # Safety
///
/// The engine keeps the rules of `patina_sdk::glue::ui`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn patina_cluster_ui(
    cluster: *const ::std::ffi::c_void,
    node: u32,
    mode: u32,
    text: *const u8,
    len: usize,
    rep: *const ::patina_sdk::glue::Reporter,
) -> bool {
    // SAFETY: the engine keeps the rules of `glue::ui`.
    unsafe { ::patina_sdk::glue::ui(cluster.cast(), node, mode, text, len, rep) }
}

/// # Safety
///
/// The engine keeps the rules of `patina_sdk::glue::parallel`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn patina_cluster_parallel(cluster: *const ::std::ffi::c_void) -> bool {
    // SAFETY: the engine keeps the rules of `glue::parallel`.
    unsafe { ::patina_sdk::glue::parallel(cluster.cast()) }
}

/// # Safety
///
/// The engine keeps the rules of `patina_sdk::glue::task`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn patina_cluster_task(
    cluster: *const ::std::ffi::c_void,
    task: u32,
    rep: *const ::patina_sdk::glue::Reporter,
) -> bool {
    // SAFETY: the engine keeps the rules of `glue::task`.
    unsafe { ::patina_sdk::glue::task(cluster.cast(), task, rep) }
}

/// # Safety
///
/// The engine keeps the rules of `patina_sdk::glue::end`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn patina_cluster_end(
    cluster: *const ::std::ffi::c_void,
    domain: u32,
    rep: *const ::patina_sdk::glue::Reporter,
) -> bool {
    // SAFETY: the engine keeps the rules of `glue::end`.
    unsafe { ::patina_sdk::glue::end(cluster.cast(), domain, rep) }
}

/// # Safety
///
/// The engine keeps the rules of `patina_sdk::glue::take`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn patina_cluster_take(
    cluster: *const ::std::ffi::c_void,
    node: u32,
    rep: *const ::patina_sdk::glue::Reporter,
) -> *mut ::std::ffi::c_void {
    // SAFETY: the engine keeps the rules of `glue::take`.
    unsafe { ::patina_sdk::glue::take(cluster.cast(), node, rep) }
}

/// # Safety
///
/// The engine keeps the rules of `patina_sdk::glue::put`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn patina_cluster_put(
    cluster: *const ::std::ffi::c_void,
    node: u32,
    value: *mut ::std::ffi::c_void,
    rep: *const ::patina_sdk::glue::Reporter,
) -> bool {
    // SAFETY: the engine keeps the rules of `glue::put`.
    unsafe { ::patina_sdk::glue::put(cluster.cast(), node, value, rep) }
}

/// # Safety
///
/// The engine keeps the rules of `patina_sdk::glue::export`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn patina_cluster_export(
    cluster: *const ::std::ffi::c_void,
    index: u32,
    rep: *const ::patina_sdk::glue::Reporter,
) -> *mut ::std::ffi::c_void {
    // SAFETY: the engine keeps the rules of `glue::export`.
    unsafe { ::patina_sdk::glue::export(cluster.cast(), index, rep) }
}

/// # Safety
///
/// The engine keeps the rules of `patina_sdk::glue::import`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn patina_cluster_import(
    cluster: *const ::std::ffi::c_void,
    index: u32,
    value: *mut ::std::ffi::c_void,
    rep: *const ::patina_sdk::glue::Reporter,
) -> bool {
    // SAFETY: the engine keeps the rules of `glue::import`.
    unsafe { ::patina_sdk::glue::import(cluster.cast(), index, value, rep) }
}

/// # Safety
///
/// The engine keeps the rules of `patina_sdk::glue::free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn patina_cluster_free(cluster: *mut ::std::ffi::c_void) {
    // SAFETY: the engine keeps the rules of `glue::free`.
    unsafe { ::patina_sdk::glue::free(cluster.cast()) }
}
"#;

/// Sends the `log` records of the nodes to the engine (15.7).
const LOGGER: &str = r#"
struct PatinaLog;

impl ::patina_log::Log for PatinaLog {
    fn enabled(&self, metadata: &::patina_log::Metadata<'_>) -> bool {
        metadata.level() <= ::patina_log::Level::Info
    }

    fn log(&self, record: &::patina_log::Record<'_>) {
        if self.enabled(record.metadata()) {
            ::patina_sdk::glue::log_line(record.level().as_str(), record.args());
        }
    }

    fn flush(&self) {}
}
"#;

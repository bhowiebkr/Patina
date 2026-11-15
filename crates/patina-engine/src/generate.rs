//! Writes the cluster crate of a plan (spec 8.2).
//!
//! The code names no type of a node crate: the parameters live in one closure, and Rust infers
//! each type from the node calls. The code uses only full paths, because a node crate can have
//! the name of any `use`.

use std::fmt::Write;

use crate::node::{Output, Pass};
use crate::plan::{Plan, Source};

pub const PACKAGE: &str = "cluster-main";
/// The Rust name of the library target of `PACKAGE`.
pub const TARGET: &str = "cluster_main";

const HEADER: &str = "# Made by Patina. Patina writes this file again at each build.\n";

pub fn manifest(plan: &Plan) -> String {
    let mut text = format!(
        "{HEADER}[package]\nname = \"{PACKAGE}\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\
         publish = false\n\n[lib]\ncrate-type = [\"cdylib\"]\npath = \"lib.rs\"\n\n\
         [dependencies]\npatina-sdk = {{ workspace = true }}\n"
    );
    let mut names: Vec<&str> = plan.nodes.iter().enumerate().filter(|(i, _)| plan.delay != Some(*i)).map(|(_, (name, _))| name.as_str()).collect();
    names.sort_unstable();
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

pub fn source(plan: &Plan) -> String {
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
    for (i, slot) in plan.slots.iter().enumerate() {
        let ty = if slot.is_str { ": ::patina_sdk::glue::Shared<::std::string::String>" } else { "" };
        let id = &plan.steps[slot.step].id;
        let _ = writeln!(
            s,
            "    let p{i}{ty} = ::patina_sdk::glue::Shared::new(::patina_sdk::glue::default_value()); // {id}.{}",
            slot.key
        );
        shared.push(format!("p{i}"));
    }
    for (n, k) in plan.held().filter(|&(n, k)| is_shared(plan, n, k)) {
        let _ = writeln!(s, "    let n{n}_{k} = ::patina_sdk::glue::Shared::new(::std::option::Option::None);");
        shared.push(format!("n{n}_{k}"));
    }
    // Each use of a stateful node has its own state (4.2). It drops with the cluster.
    for (n, step) in plan.steps.iter().enumerate().filter(|(_, step)| plan.nodes[step.node].1.state.is_some()) {
        let _ = writeln!(s, "    let s{n} = ::patina_sdk::glue::State::new(); // {}", step.id);
        shared.push(format!("s{n}"));
    }
    // The value of each delay for the next frame (9.2). The first frame gives `Default::default()`.
    for (n, step) in plan.steps.iter().enumerate().filter(|&(n, _)| plan.is_delay(n)) {
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
    for (i, slot) in plan.slots.iter().enumerate() {
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
    let mut order: Vec<usize> = (0..plan.tasks.len()).collect();
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
    // The end of a domain drops the values of its steps (9.1). A value that another domain reads
    // stays until its step runs again.
    for domain in plan.tasks.iter().map(|t| t.domain).collect::<std::collections::BTreeSet<_>>() {
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
    }
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

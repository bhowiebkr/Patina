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
    let mut names: Vec<&str> = plan.nodes.iter().map(|(name, _)| name.as_str()).collect();
    names.sort_unstable();
    for name in names {
        let _ = writeln!(text, "{name} = {{ path = \"../../../nodes/{name}\" }}");
    }
    text
}

pub fn source(plan: &Plan) -> String {
    let mut s = String::from(
        "// Made by Patina. Patina writes this file again at each build.\n#![allow(unused)]\n\n\
         use ::patina_sdk::glue::{DebugText as _, NameText as _, ViewerText as _};\n\n\
         fn make() -> ::patina_sdk::glue::Cluster {\n",
    );
    for (i, slot) in plan.slots.iter().enumerate() {
        let ty = if slot.is_str { ": ::std::string::String" } else { "" };
        let id = &plan.steps[slot.step].id;
        let _ = writeln!(s, "    let mut p{i}{ty} = ::patina_sdk::glue::default_value(); // {id}.{}", slot.key);
    }
    s.push_str(
        "    ::patina_sdk::glue::cluster(move |cmd, rep| match cmd {\n        \
         ::patina_sdk::glue::Cmd::Set(index, text) => match index {\n",
    );
    for (i, slot) in plan.slots.iter().enumerate() {
        let _ = writeln!(s, "            {i} => ::patina_sdk::glue::set_param(&mut p{i}, text, rep, {}),", slot.step);
    }
    s.push_str("            _ => false,\n        },\n        ::patina_sdk::glue::Cmd::Run => {\n");
    s.push_str("            let mut ok = true;\n");
    for (n, k) in plan.held() {
        let _ = writeln!(s, "            let mut n{n}_{k} = ::std::option::Option::None;");
    }
    let mut probe = 0;
    for n in 0..plan.steps.len() {
        step(&mut s, plan, n, &mut probe);
    }
    for (n, k) in plan.held() {
        let _ = writeln!(s, "            ok &= ::patina_sdk::glue::drop_value(n{n}_{k}, rep, {n});");
    }
    s.push_str("            ok\n        }\n    })\n}\n");
    s.push_str(EXPORTS);
    s
}

/// Writes the call of one node. It runs only when each wired input has a value (15.3). Then it
/// sends the text of each held value, before a later node takes the value (15.9).
fn step(s: &mut String, plan: &Plan, n: usize, probe: &mut usize) {
    let step = &plan.steps[n];
    let (name, node) = &plan.nodes[step.node];
    let wired: Vec<String> = step
        .inputs
        .iter()
        .filter_map(|source| match source {
            Source::Wire { step, output } => Some(format!("n{step}_{output}.is_some()")),
            Source::Slot(_) => None,
        })
        .collect();
    let _ = writeln!(s, "            // {}: nodes/{name}", step.id);
    let indent = if wired.is_empty() {
        "            "
    } else {
        let _ = writeln!(s, "            if {} {{", wired.join(" && "));
        "                "
    };
    let _ = writeln!(
        s,
        "{indent}ok &= ::patina_sdk::glue::finish(::std::panic::catch_unwind(\
         ::std::panic::AssertUnwindSafe(|| -> ::std::result::Result<(), ::std::string::String> {{"
    );
    let inner = format!("{indent}    ");
    let mut args = Vec::new();
    for (input, source) in node.inputs.iter().zip(&step.inputs) {
        let value = match (source, input.pass) {
            (Source::Slot(i), Pass::Ref) => format!("&p{i}"),
            (Source::Slot(i), _) => format!("::std::clone::Clone::clone(&p{i})"),
            (Source::Wire { step: from, output }, Pass::Ref) => format!("n{from}_{output}.as_ref().unwrap()"),
            // Only one node reads the value, so it takes the value with no copy.
            (Source::Wire { step: from, output }, _) if plan.steps[*from].consumers[*output] == 1 => {
                format!("n{from}_{output}.take().unwrap()")
            }
            (Source::Wire { step: from, output }, _) => {
                format!("::std::clone::Clone::clone(n{from}_{output}.as_ref().unwrap())")
            }
        };
        if input.pass == Pass::Mut {
            let _ = writeln!(s, "{inner}let mut m_{} = {value};", input.key);
            args.push(format!("&mut m_{}", input.key));
        } else {
            args.push(value);
        }
    }
    let call = format!("::{}::{}({})", name.replace('-', "_"), node.func, args.join(", "));
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
            let _ = writeln!(s, "{inner}n{n}_{k} = ::std::option::Option::Some({value});");
        } else {
            let _ = writeln!(s, "{inner}::std::mem::drop({value});");
        }
    }
    let _ = writeln!(s, "{inner}::std::result::Result::Ok(())\n{indent}}})), rep, {n});");
    for k in (0..values.len()).filter(|k| step.consumers[*k] > 0) {
        let _ = writeln!(
            s,
            "{indent}if let ::std::option::Option::Some(v) = &n{n}_{k} {{ ::patina_sdk::glue::send_value(rep, {probe}, \
             |out| (&&&::patina_sdk::glue::Probe(v)).text(out)); }}"
        );
        *probe += 1;
    }
    if !wired.is_empty() {
        s.push_str("            }\n");
    }
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
/// The engine keeps the rules of `patina_sdk::glue::run`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn patina_cluster_run(
    cluster: *mut ::std::ffi::c_void,
    rep: *const ::patina_sdk::glue::Reporter,
) -> bool {
    // SAFETY: the engine keeps the rules of `glue::run`.
    unsafe { ::patina_sdk::glue::run(cluster.cast(), rep) }
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

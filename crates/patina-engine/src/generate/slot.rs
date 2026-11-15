//! Writes the slot (spec 7.1): the call of the base into the slot library, and the crate of the
//! slot library. The slot library holds only the node in the slot, and the state of each use.

use std::fmt::Write;

use super::{EXPORTS, HEADER, LOGGER};
use crate::node::Pass;
use crate::plan::Plan;

/// The package name of the slot library. Its folder is `build/clusters/slot`.
pub const PACKAGE: &str = "cluster-slot";

/// The Rust name of the library target of `PACKAGE`.
pub const TARGET: &str = "cluster_slot";

/// Writes the call of step `n` into the slot library. A block that never runs gives Rust the type
/// of the result, so the code names no type.
pub(super) fn call(s: &mut String, plan: &Plan, n: usize) {
    let (name, node) = &plan.nodes[plan.steps[n].node];
    let krate = name.replace('-', "_");
    let indent = "                ";
    let _ = writeln!(s, "{indent}let t = ::std::time::Instant::now();");
    let (mut args, mut pointers) = (Vec::new(), Vec::new());
    for (j, (input, value)) in node.inputs.iter().zip(super::inputs(plan, n)).enumerate() {
        match input.pass {
            Pass::Ref => {
                pointers.push(format!("::patina_sdk::glue::arg({value})"));
                args.push(value);
            }
            Pass::Take => {
                let _ = writeln!(s, "{indent}let mut a{j} = ::std::option::Option::Some({value});");
                args.push(format!("a{j}.take().unwrap()"));
                pointers.push(format!("::patina_sdk::glue::arg_mut(&mut a{j})"));
            }
            Pass::Mut => {
                let _ = writeln!(s, "{indent}let mut m_{} = {value};", input.key);
                args.push(format!("&mut m_{}", input.key));
                pointers.push(format!("::patina_sdk::glue::arg_mut(&mut m_{})", input.key));
            }
        }
    }
    let mut call = format!("::{krate}::{}", node.func);
    if let Some(state) = &node.state {
        args.insert(0, format!("&mut ::{krate}::{state}::init()"));
        call = format!("::{krate}::{state}::{}", node.func);
    }
    let mut call = format!("{call}({})", args.join(", "));
    if node.fallible {
        call = format!("match {call} {{ ::std::result::Result::Ok(v) => v, ::std::result::Result::Err(_) => ::std::unreachable!() }}");
    }
    pointers.push("::patina_sdk::glue::arg_mut(&mut r)".to_owned());
    let _ = writeln!(s, "{indent}let mut r = ::std::option::Option::None;");
    let _ = writeln!(s, "{indent}if false {{ r = ::std::option::Option::Some({call}); }}");
    let _ = writeln!(s, "{indent}ok &= slot.call({n}, &[{}], rep);", pointers.join(", "));
    let _ = writeln!(s, "{indent}rep.time({n}, t);");
    let _ = writeln!(s, "{indent}if let ::std::option::Option::Some(r) = r {{");
    super::store(s, plan, n, &format!("{indent}    "));
    let _ = writeln!(s, "{indent}}}");
}

/// The manifest of the slot library. `plan.slot` is the node in the slot.
pub fn manifest(plan: &Plan, slot: usize) -> String {
    let name = &plan.nodes[slot].0;
    let mut text = format!(
        "{HEADER}[package]\nname = \"{PACKAGE}\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\
         publish = false\n\n[lib]\ncrate-type = [\"cdylib\"]\npath = \"lib.rs\"\n\n\
         [dependencies]\npatina-sdk = {{ workspace = true }}\n{name} = {{ path = \"../../../nodes/{name}\" }}\n"
    );
    if plan.log {
        text.push_str("patina-log = { package = \"log\", version = \"0.4\" }\n");
    }
    text
}

/// The source of the slot library. Each use of the node is one `Call` arm, with its own state.
pub fn source(plan: &Plan, slot: usize) -> String {
    let (name, node) = &plan.nodes[slot];
    let krate = name.replace('-', "_");
    let uses: Vec<usize> = (0..plan.steps.len()).filter(|&n| plan.in_slot(n)).collect();
    let stateful: Vec<usize> = uses.iter().copied().filter(|_| node.state.is_some()).collect();
    let mut s = String::from(
        "// Made by Patina. Patina writes this file again at each build.\n#![allow(unused)]\n\n\
         use ::patina_sdk::glue::{NotSyncCheck as _, SyncCheck as _};\n\n\
         fn make() -> ::patina_sdk::glue::Cluster {\n",
    );
    if plan.log {
        s.push_str("    let _ = ::patina_log::set_logger(&PatinaLog);\n");
        s.push_str("    ::patina_log::set_max_level(::patina_log::LevelFilter::Info);\n");
    }
    for &n in &stateful {
        let _ = writeln!(s, "    let s{n} = ::patina_sdk::glue::State::new(); // {}", plan.steps[n].id);
        let _ = writeln!(s, "    let c_s{n} = ::patina_sdk::glue::check(&s{n});");
    }
    s.push_str(
        "    let f = ::patina_sdk::glue::typed(move |cmd, rep| match cmd {\n        \
         ::patina_sdk::glue::Cmd::Call(index, args) => match index {\n",
    );
    let count = node.inputs.len();
    for &n in &uses {
        // A base of another build can give other arguments, so the count must match.
        let _ = writeln!(s, "            {n} if args.len() == {} => {{", count + 1);
        if node.state.is_some() {
            let _ = writeln!(s, "                let mut st = s{n}.lock();");
        }
        let _ = writeln!(
            s,
            "                ::patina_sdk::glue::{}(::std::panic::catch_unwind(\
             ::std::panic::AssertUnwindSafe(|| -> ::std::result::Result<(), ::std::string::String> {{",
            if node.state.is_some() { "finish_state" } else { "finish" }
        );
        let inner = "                    ";
        let mut args = Vec::new();
        let mut call = format!("::{krate}::{}", node.func);
        if let Some(state) = &node.state {
            let _ = writeln!(s, "{inner}let st = st.get_or_insert_with(::{krate}::{state}::init);");
            args.push("st".to_owned());
            call = format!("::{krate}::{state}::{}", node.func);
        }
        for (j, input) in node.inputs.iter().enumerate() {
            let value = match input.pass {
                Pass::Ref if input.is_str => format!("::patina_sdk::glue::read_arg::<::std::string::String>(args[{j}]).as_str()"),
                Pass::Ref => format!("::patina_sdk::glue::read_arg(args[{j}])"),
                Pass::Take => format!("::patina_sdk::glue::take_arg(args[{j}])"),
                Pass::Mut => format!("::patina_sdk::glue::mut_arg(args[{j}])"),
            };
            let _ = writeln!(s, "{inner}// SAFETY: the base gives the inputs in this order (generate/slot.rs).");
            let _ = writeln!(s, "{inner}let a{j} = unsafe {{ {value} }};");
            args.push(format!("a{j}"));
        }
        let call = format!("{call}({})", args.join(", "));
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
        let _ = writeln!(s, "{inner}// SAFETY: the last argument is the `Option` of the result (generate/slot.rs).");
        let _ = writeln!(s, "{inner}unsafe {{ ::patina_sdk::glue::put_result(args[{count}], r) }};");
        let state = if node.state.is_some() { "&mut *st, " } else { "" };
        let _ = writeln!(s, "{inner}::std::result::Result::Ok(())\n                }})), {state}rep, {n})\n            }}");
    }
    s.push_str("            _ => false,\n        },\n        ::patina_sdk::glue::Cmd::Ui(index, mode, events) => match index {\n");
    super::ui_arms(&mut s, plan, &stateful);
    s.push_str("            _ => false,\n        },\n        ::patina_sdk::glue::Cmd::Take(index, out) => match index {\n");
    for n in &stateful {
        let _ = writeln!(s, "            {n} => ::patina_sdk::glue::take_state(&s{n}, out),");
    }
    s.push_str("            _ => false,\n        },\n        ::patina_sdk::glue::Cmd::Put(index, moved) => match index {\n");
    for n in &stateful {
        let _ = writeln!(s, "            {n} => ::patina_sdk::glue::put_state(&s{n}, moved, rep, {n}),");
    }
    s.push_str("            _ => false,\n        },\n        _ => false,\n    });\n    let parallel = true");
    for n in &stateful {
        let _ = write!(s, " && (&&c_s{n}).sync()");
    }
    s.push_str(";\n    ::patina_sdk::glue::cluster(f, parallel)\n}\n");
    s.push_str(EXPORTS);
    if plan.log {
        s.push_str(LOGGER);
    }
    s
}

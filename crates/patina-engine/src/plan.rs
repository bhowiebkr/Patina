//! Checks a graph and puts its nodes in run order (spec 6.4, 9.2).

use std::collections::BTreeSet;
use std::path::Path;

use read::{Entry, Read};
use task::{is_light, sort, split};

use crate::node::{self, Builtin, Domain, Kind, Node};
use crate::project::{GraphFile, read_crate};
use crate::types;

mod domain;
mod event;
mod read;
mod task;

/// A graph that passed all checks, in run order.
#[derive(Clone)]
pub struct Plan {
    /// The node folders that the graph uses, by folder name. The name is the crate name (6.2).
    pub nodes: Vec<(String, Node)>,
    pub steps: Vec<Step>,
    /// One parameter slot for each input with no wire.
    pub slots: Vec<Slot>,
    /// The connection kinds of the held values, by folder name (6.3). The name is the crate name.
    pub kinds: Vec<(String, Kind)>,
    /// True when a crate folder of the plan has the `log` crate. The cluster then sends the log
    /// lines of its nodes (15.7).
    pub log: bool,
    /// The tasks of the scheduler (9.2), in run order.
    pub tasks: Vec<Task>,
    /// The index in `nodes` of each built-in node that the graph uses (9.2, 9.4).
    pub builtins: Vec<(usize, Builtin)>,
    /// The index in `nodes` of the node in the slot (7.1). Each of its uses runs in the slot
    /// library.
    pub slot: Option<usize>,
}

/// A chain of steps that one call runs in order (9.2).
#[derive(Clone)]
pub struct Task {
    pub steps: Vec<usize>,
    /// The number of tasks that must end before this task starts.
    pub inputs: u32,
    /// The tasks that wait for this task.
    pub next: Vec<usize>,
    pub domain: Domain,
}

/// One use of a node folder (6.2).
#[derive(Clone)]
pub struct Step {
    pub id: String,
    /// The index in `Plan::nodes`.
    pub node: usize,
    /// One source for each input of the node, in order.
    pub inputs: Vec<Source>,
    /// The number of wires from each output, in the order of `Node::output_keys`.
    pub consumers: Vec<usize>,
    /// The steps that read each output, one entry for each wire, in the order of `consumers`.
    pub readers: Vec<Vec<usize>>,
    /// The type of each output, with the types of the wires in the parameters of a generic node (4.5).
    pub types: Vec<String>,
    /// The steps that read an output and give a type parameter that no input wire gives (4.5).
    pub pins: Vec<usize>,
    /// The index in `Plan::tasks`.
    pub task: usize,
    pub domain: Domain,
}

impl Plan {
    /// Makes the output `from`, "<step ID>.<output key>", send its value with no wire, as the
    /// public output of a test does (15.9). A wire count of 1 with no reader moves no value.
    /// Probes the inner output of each alias: the cluster sends only the values of outputs with a
    /// wire, and a public output of a test use has none (15.9).
    pub(crate) fn probe_all(&mut self, aliases: &[(String, String)]) {
        for (_, inner) in aliases {
            self.probe(inner);
        }
    }

    pub(crate) fn probe(&mut self, from: &str) {
        let Some((id, key)) = from.rsplit_once('.') else { return };
        let Some(step) = self.steps.iter_mut().find(|s| s.id == id) else { return };
        if let Some(k) = self.nodes[step.node].1.output_keys().iter().position(|o| o == key) {
            step.consumers[k] = step.consumers[k].max(1);
        }
    }

    /// Each output that a wire reads, as a step index and an output index, in run order.
    pub fn held(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        self.steps.iter().enumerate().flat_map(|(n, step)| {
            step.consumers.iter().enumerate().filter(|(_, c)| **c > 0).map(move |(k, _)| (n, k))
        })
    }

    /// The index of each step of a stateful node (4.2).
    pub fn stateful(&self) -> impl Iterator<Item = usize> + '_ {
        (0..self.steps.len()).filter(|&n| self.nodes[self.steps[n].node].1.state.is_some())
    }

    /// The built-in node of step `n` (9.2, 9.4), or None for a use of a node folder.
    pub fn builtin(&self, n: usize) -> Option<Builtin> {
        self.builtins.iter().find(|(i, _)| *i == self.steps[n].node).map(|&(_, b)| b)
    }

    /// True when step `n` is a delay node (9.2).
    pub fn is_delay(&self, n: usize) -> bool {
        self.builtin(n) == Some(Builtin::Delay)
    }

    /// True when step `n` is a light node (9.2): it reads and gives only number values.
    pub fn is_light(&self, n: usize) -> bool {
        is_light(&self.steps, &self.nodes, n, self.is_delay(n))
    }

    /// True when step `n` is a trigger that starts event runs (9.4).
    pub fn is_trigger(&self, n: usize) -> bool {
        self.builtin(n).is_some_and(Builtin::fires)
    }

    /// True when the node with index `i` in `nodes` is built in.
    pub fn is_builtin_node(&self, i: usize) -> bool {
        self.builtins.iter().any(|&(b, _)| b == i)
    }

    /// True when step `n` is a use of the node in the slot (7.1).
    pub fn in_slot(&self, n: usize) -> bool {
        self.slot == Some(self.steps[n].node)
    }

    /// The connection kind of output `k` of step `n`, when a kind shows its type (6.3).
    pub fn kind(&self, n: usize, k: usize) -> Option<&(String, Kind)> {
        let ty = &self.steps[n].types[k];
        self.kinds.iter().find(|(_, kind)| &kind.ty == ty)
    }
}

#[derive(Clone)]
pub enum Source {
    Slot(usize),
    Wire { step: usize, output: usize },
}

#[derive(Clone)]
pub struct Slot {
    pub step: usize,
    /// The key of the input.
    pub key: String,
    pub is_str: bool,
}

/// Checks a graph of the project at `root`. Returns all the problems that it finds.
pub fn check(root: &Path, graph: &GraphFile) -> Result<Plan, Vec<String>> {
    let mut problems = Vec::new();
    let kinds = read_kinds(root, &mut problems);
    let mut read = Read::new(root, graph, &mut problems);
    let edges = read.wires(graph, &mut problems);
    if !problems.is_empty() {
        return Err(problems);
    }
    let Read { nodes, entries, builtins, .. } = read;
    let is_delay = |node: usize| read::is_delay(&builtins, node);
    let fires = |node: usize| read::fires(&builtins, node);
    let entries: Vec<Entry> = entries.into_iter().map(|e| e.expect("no problems")).collect();
    let order = sort(entries.len(), &edges).map_err(|left| {
        let ids: Vec<&str> = left.iter().map(|&i| entries[i].id.as_str()).collect();
        vec![format!("the graph has a cycle through these nodes: {}", ids.join(", "))]
    })?;
    let mut step_of = vec![0; entries.len()];
    for (step, &entry) in order.iter().enumerate() {
        step_of[entry] = step;
    }
    let (mut steps, slots) = make_steps(&order, &step_of, &entries, &nodes, is_delay)?;
    let unwired = needs_wire(root, &steps, &slots, &nodes, |n| builtins.iter().any(|&(i, _)| i == n));
    if !unwired.is_empty() {
        return Err(unwired);
    }
    resolve(&mut steps, &nodes, fires)?;
    let edges: Vec<(usize, usize)> = edges.iter().map(|&(from, to)| (step_of[from], step_of[to])).collect();
    let own: Vec<Option<Domain>> = order.iter().map(|&e| entries[e].domain).collect();
    domain::assign(&mut steps, &edges, &own, is_delay)?;
    let trigger = event::triggers(&steps, &edges, |n| fires(steps[n].node))?;
    let light: Vec<bool> = (0..steps.len()).map(|n| is_light(&steps, &nodes, n, is_delay(steps[n].node))).collect();
    let tasks = split(&mut steps, &edges, &trigger, &light);
    let mut plan = Plan { nodes, steps, slots, kinds, log: false, tasks, builtins, slot: None };
    let used: BTreeSet<String> = plan.held().filter_map(|(n, k)| plan.kind(n, k)).map(|(name, _)| name.clone()).collect();
    plan.kinds.retain(|(name, _)| used.contains(name));
    let mut folders = plan.nodes.iter().enumerate().filter(|(i, _)| !plan.is_builtin_node(*i)).map(|(_, (n, _))| ("nodes", n)).chain(plan.kinds.iter().map(|(n, _)| ("connections", n)));
    plan.log = folders.any(|(group, name)| uses_log(root, group, name));
    Ok(plan)
}

/// Makes one step for each entry in run `order`, with the source of each input. `step_of` gives
/// the step of each entry. An input with no wire gets a slot for its value.
fn make_steps(order: &[usize], step_of: &[usize], entries: &[Entry], nodes: &[(String, Node)], is_delay: impl Fn(usize) -> bool) -> Result<(Vec<Step>, Vec<Slot>), Vec<String>> {
    let mut steps: Vec<Step> = order
        .iter()
        .map(|&e| Step {
            id: entries[e].id.clone(),
            node: entries[e].node,
            inputs: Vec::new(),
            consumers: vec![0; nodes[entries[e].node].1.output_keys().len()],
            readers: vec![Vec::new(); nodes[entries[e].node].1.output_keys().len()],
            types: Vec::new(),
            pins: Vec::new(),
            task: 0,
            domain: Domain::Batch,
        })
        .collect();
    let mut slots = Vec::new();
    for (step, &e) in order.iter().enumerate() {
        let entry = &entries[e];
        for (input, wired) in nodes[entry.node].1.inputs.iter().zip(&entry.wired) {
            let source = match *wired {
                Some((from, output)) => {
                    steps[step_of[from]].consumers[output] += 1;
                    steps[step_of[from]].readers[output].push(step);
                    Source::Wire { step: step_of[from], output }
                }
                None if is_delay(entry.node) => {
                    // The page shows a node with only this problem as unresolved (15.2), so keep the start of the text.
                    return Err(vec![format!("node {}: connect a wire to the input `value` of the delay (spec 9.2)", entry.id)]);
                }
                None => {
                    slots.push(Slot { step, key: input.key.clone(), is_str: input.is_str });
                    Source::Slot(slots.len() - 1)
                }
            };
            steps[step].inputs.push(source);
        }
    }
    Ok((steps, slots))
}

/// The problems of the inputs with no wire whose type has no parameter support (4.8). The wires
/// give the type of a type parameter (4.5), and a built-in node has only parameter types.
fn needs_wire(root: &Path, steps: &[Step], slots: &[Slot], nodes: &[(String, Node)], builtin: impl Fn(usize) -> bool) -> Vec<String> {
    let mut problems = Vec::new();
    for slot in slots {
        let step = &steps[slot.step];
        let (name, node) = &nodes[step.node];
        let Some(input) = node.inputs.iter().find(|i| i.key == slot.key) else { continue };
        let generic = input.value.split(|c: char| !c.is_alphanumeric() && c != '_').any(|w| node.generics.iter().any(|g| g == w));
        if !(builtin(step.node) || input.is_str || generic || has_param(root, name, &input.value)) {
            problems.push(format!("node {}: connect a wire to the input `{}`. Its type `{}` has no parameter support (spec 4.8)", step.id, input.key, input.value));
        }
    }
    problems
}

/// True when a parameter can hold a value of `ty`: a scalar, a `String`, or a type with an
/// `impl Param for` it in the node crate or in a type crate of the project (4.8).
fn has_param(root: &Path, node: &str, ty: &str) -> bool {
    let name = ty.rsplit("::").next().unwrap_or(ty);
    if scalar(ty) || name == "String" {
        return true;
    }
    let name = name.split('<').next().unwrap_or(name).trim();
    let mut dirs = vec![root.join("nodes").join(node).join("src")];
    if let Ok(types) = std::fs::read_dir(root.join("types")) {
        dirs.extend(types.filter_map(Result::ok).map(|e| e.path().join("src")));
    }
    while let Some(dir) = dirs.pop() {
        for path in std::fs::read_dir(&dir).into_iter().flatten().filter_map(Result::ok).map(|e| e.path()) {
            if path.is_dir() {
                dirs.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") && std::fs::read_to_string(&path).is_ok_and(|code| impls_param(&code, name)) {
                return true;
            }
        }
    }
    false
}

/// True when `code` has `impl Param for <name>`, also with a path before `Param` or `<name>`.
fn impls_param(code: &str, name: &str) -> bool {
    let word = |c: char| c.is_alphanumeric() || c == '_';
    code.match_indices("Param").any(|(at, m)| {
        let Some(after) = code[at + m.len()..].trim_start().strip_prefix("for") else { return false };
        let path: String = after.trim_start().chars().take_while(|&c| word(c) || c == ':').collect();
        !code[..at].ends_with(word) && after.starts_with(char::is_whitespace) && path.rsplit("::").next() == Some(name)
    })
}

/// Sets the output types of each step. The wires give the type parameters of a generic node (4.5):
/// first the input wires, then the wires from the outputs to inputs of a known type. A trigger does
/// not run its inputs in the witness (8.2), so it gives no type. The compiler then makes a copy of
/// the node for each set of types. A delay comes before the node of its input (9.2), so a step that
/// waits for a type goes to the next pass.
fn resolve(steps: &mut [Step], nodes: &[(String, Node)], fires: impl Fn(usize) -> bool) -> Result<(), Vec<String>> {
    let mut problems = Vec::new();
    let mut done = vec![false; steps.len()];
    let mut progress = true;
    while progress {
        progress = false;
        for n in 0..steps.len() {
            if done[n] {
                continue;
            }
            let node = &nodes[steps[n].node].1;
            let mut bound = vec![None; node.generics.len()];
            let (mut waits, mut conflicts) = (false, Vec::new());
            for (input, source) in node.inputs.iter().zip(&steps[n].inputs) {
                let Source::Wire { step, output } = *source else { continue };
                waits |= !done[step];
                if let Some(actual) = steps[step].types.get(output)
                    && let Err(text) = types::unify(&input.value, actual, &node.generics, &mut bound)
                {
                    conflicts.push(format!("node {}: the wires give two types: {text}", steps[n].id));
                }
            }
            if waits && bound.contains(&None) {
                continue;
            }
            (done[n], progress) = (true, true);
            let (mut pins, open) = (Vec::new(), bound.contains(&None));
            for (r, reader) in steps.iter().enumerate().filter(|(_, s)| open && !fires(s.node)) {
                let to = &nodes[reader.node].1;
                for (input, source) in to.inputs.iter().zip(&reader.inputs) {
                    let Source::Wire { step, output } = *source else { continue };
                    if step != n || types::mentions(&input.value, &to.generics) {
                        continue;
                    }
                    pins.push(r);
                    if let Err(text) = types::unify(&node.output_types[output], &input.value, &node.generics, &mut bound) {
                        conflicts.push(format!("node {}: the wires give two types: {text}", steps[n].id));
                    }
                }
            }
            steps[n].pins = pins;
            problems.extend(conflicts);
            let open: Vec<&str> = node.generics.iter().zip(&bound).filter(|(_, b)| b.is_none()).map(|(g, _)| g.as_str()).collect();
            if !open.is_empty() {
                // The page shows a node with only this problem as unresolved (15.2), so keep the start of the text.
                problems.push(format!("node {}: connect a wire that gives the type of {} (spec 4.5)", steps[n].id, open.join(", ")));
                continue;
            }
            let bound: Vec<String> = bound.into_iter().flatten().collect();
            steps[n].types = node.output_types.iter().map(|ty| types::substitute(ty, &node.generics, &bound)).collect();
        }
    }
    for n in (0..steps.len()).filter(|&n| !done[n]) {
        let open = nodes[steps[n].node].1.generics.join(", ");
        // The page shows a node with only this problem as unresolved (15.2), so keep the start of the text.
        problems.push(format!("node {}: connect a wire that gives the type of {open} (spec 4.5). A cycle of generic nodes gives none", steps[n].id));
    }
    if problems.is_empty() { Ok(()) } else { Err(problems) }
}

/// True when a folder depends on the `log` crate (15.7), also in a target table or with another name.
fn uses_log(root: &Path, group: &str, name: &str) -> bool {
    let text = std::fs::read_to_string(root.join(group).join(name).join("Cargo.toml")).unwrap_or_default();
    let manifest: toml::Table = text.parse().unwrap_or_default();
    let is_log = |(key, dep): (&String, &toml::Value)| dep.get("package").and_then(|p| p.as_str()).unwrap_or(key) == "log";
    let has_log = |t: &toml::Table| t.get("dependencies").and_then(|d| d.as_table()).is_some_and(|d| d.iter().any(is_log));
    has_log(&manifest) || manifest.get("target").and_then(|t| t.as_table()).is_some_and(|t| t.values().filter_map(|v| v.as_table()).any(has_log))
}

/// Reads each connection kind in `connections/` (6.3), sorted by folder name. A kind folder with
/// an error is a problem of the graph, because the cargo workspace of the project contains it.
pub(crate) fn read_kinds(root: &Path, problems: &mut Vec<String>) -> Vec<(String, Kind)> {
    let mut names: Vec<String> = match std::fs::read_dir(root.join("connections")) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .filter(|e| e.path().is_dir())
            .filter_map(|e| e.file_name().into_string().ok())
            .collect(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => {
            problems.push(format!("connections: {e}"));
            Vec::new()
        }
    };
    names.sort_unstable();
    let mut kinds: Vec<(String, Kind)> = Vec::new();
    for name in names {
        let kind = read_crate(root, "connections", &name).and_then(|code| node::parse_kind(&code).map_err(|e| format!("src/lib.rs: {e}")));
        let problem = match kind {
            Err(text) => text,
            // Cargo refuses two packages with the same name in one workspace.
            Ok(_) if root.join("nodes").join(&name).is_dir() => "a node folder has the same name. Rename one of them".to_owned(),
            Ok(kind) => match kinds.iter().find(|(_, k)| k.ty == kind.ty) {
                Some((other, _)) => format!("connections/{other} also shows the type {}", kind.ty),
                None => {
                    kinds.push((name, kind));
                    continue;
                }
            },
        };
        problems.push(format!("connections/{name}: {problem}"));
    }
    kinds
}

/// True for a `bool`, `char` or number type, also behind `&` or `&mut`.
fn scalar(ty: &str) -> bool {
    const SCALARS: [&str; 16] = ["bool", "char", "f32", "f64", "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize"];
    SCALARS.contains(&ty.trim_start_matches('&').trim_start_matches("mut ").trim())
}

/// True for a scalar type of 64 bits or fewer. Its value can go between tasks without a lock (9.2).
pub fn is_word(ty: &str) -> bool {
    scalar(ty) && !ty.contains("128")
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::too_many_lines)]
    fn graph_problems_are_all_reported() {
        let root = std::env::temp_dir().join(format!("patina-plan-{}", std::process::id()));
        let crates = [
            ("nodes/num", "node", "pub fn num(v: f64) -> f64 { v }", ""),
            ("nodes/show", "node", "pub fn show(v: &f64) {}", "[dependencies]\nlog = \"0.4\"\n"),
            ("nodes/tick", "node(domain = frame)", "pub fn tick() -> f64 { 0.0 }", ""),
            ("nodes/bits", "node", "pub fn bits() -> u8 { 0 }", ""),
            ("nodes/pick", "node", "pub fn pick<T: Copy>(a: T, b: &T) -> (T, Vec<T>) { todo!() }", ""),
            ("nodes/zero", "node", "pub fn zero<T: Default>() -> T { T::default() }", ""),
            ("nodes/count", "node", "pub fn count(n: u8) {}", ""),
            ("connections/wide", "connection", "pub fn wide(v: &f64) -> Lanes { todo!() }", ""),
            ("connections/byte", "connection", "pub fn byte(v: &u8) -> Lanes { todo!() }", ""),
        ];
        for (folder, attr, code, more) in crates {
            let dir = root.join(folder);
            std::fs::create_dir_all(dir.join("src")).unwrap();
            let name = &folder[folder.find('/').unwrap() + 1..];
            std::fs::write(dir.join("Cargo.toml"), format!("[package]\nname = \"{name}\"\n{more}")).unwrap();
            std::fs::write(dir.join("src/lib.rs"), format!("#[patina_sdk::{attr}]\n{code}")).unwrap();
        }
        let graph = |wires: &str| -> GraphFile {
            let nodes = "format = 1\n[[node]]\nid = \"000000000000000a\"\nfolder = \"nodes/num\"\n\
                         [[node]]\nid = \"000000000000000b\"\nfolder = \"nodes/show\"\n";
            toml::from_str(&format!("{nodes}{wires}")).unwrap()
        };
        let mut good = graph("[[wire]]\nfrom = \"000000000000000a.out\"\nto = \"000000000000000b.v\"\n");
        // A lost parameter (an input that the code removed) is no problem (15.8).
        good.node[0].params.insert("gone".to_owned(), "1.0".to_owned());
        let good = check(&root, &good);
        let bad = check(
            &root,
            &graph(
                "[[wire]]\nfrom = \"000000000000000a.nope\"\nto = \"000000000000000b.v\"\n\
                 [[wire]]\nfrom = \"000000000000000a.out\"\nto = \"000000000000000b.v\"\n\
                 [[wire]]\nfrom = \"000000000000000a.out\"\nto = \"000000000000000b.v\"\n",
            ),
        );
        let cycle = check(&root, &graph("[[order]]\nbefore = \"000000000000000b\"\nafter = \"000000000000000b\"\n"));
        // tick (frame) -> show. `more` adds to the show node.
        let frames = |more: &str| {
            let text = format!(
                "format = 1\n[[node]]\nid = \"000000000000000c\"\nfolder = \"nodes/tick\"\n[[node]]\nid = \"000000000000000b\"\n\
                 folder = \"nodes/show\"\n{more}\n[[wire]]\nfrom = \"000000000000000c.out\"\nto = \"000000000000000b.v\"\n"
            );
            check(&root, &toml::from_str(&text).unwrap())
        };
        let mixed = "[[node]]\nid = \"000000000000000a\"\nfolder = \"nodes/num\"\n[[order]]\nbefore = \"000000000000000a\"\nafter = \"000000000000000b\"";
        let (frame, crossed, mixed) = (frames(""), frames("domain = \"batch\""), frames(mixed));
        let heard = frames("domain = \"audio\"");
        let mut latest = graph("[[wire]]\nfrom = \"000000000000000a.out\"\nto = \"000000000000000b.v\"\n");
        latest.node[1].domain = Some("frame".to_owned());
        let latest = check(&root, &latest);
        // The wires to pick give its type parameter (4.5): bits d gives u8, num a gives f64.
        let generic = |wires: &str| {
            let text = format!(
                "format = 1\n[[node]]\nid = \"000000000000000a\"\nfolder = \"nodes/num\"\n[[node]]\nid = \"000000000000000d\"\n\
                 folder = \"nodes/bits\"\n[[node]]\nid = \"000000000000000e\"\nfolder = \"nodes/pick\"\n{wires}"
            );
            check(&root, &toml::from_str(&text).unwrap())
        };
        let wire = |from: &str, to: &str| format!("[[wire]]\nfrom = \"00000000000000{from}\"\nto = \"000000000000000e.{to}\"\n");
        let resolved = generic(&(wire("0d.out", "a") + &wire("0d.out", "b")));
        let (two, open) = (generic(&(wire("0d.out", "a") + &wire("0a.out", "b"))), generic(""));
        // A delay f closes the cycle pick e -> f -> e (9.2). Num a gives the type of the cycle.
        let delay = |more: &str| {
            let to_f = "[[wire]]
from = \"000000000000000e.out0\"
to = \"000000000000000f.value\"
";
            generic(&format!("{}{to_f}[[node]]
id = \"000000000000000f\"
builtin = \"delay\"
{more}", wire("0f.out", "b")))
        };
        let (looped, untyped, unknown) = (delay(&wire("0a.out", "a")), delay(&wire("0f.out", "a")), generic("[[node]]
id = \"0000000000000010\"
builtin = \"echo\"
"));
        let unwired = generic("[[node]]
id = \"000000000000000f\"
builtin = \"delay\"
");
        // Zero 11 has no input, so the readers of its output give its type: num a takes f64.
        let zero = |more: &str| {
            let to_a = "[[wire]]
from = \"0000000000000011.out\"
to = \"000000000000000a.v\"
";
            generic(&format!("[[node]]
id = \"0000000000000011\"
folder = \"nodes/zero\"
{to_a}{more}"))
        };
        let backward = zero(&(wire("11.out", "a") + &wire("11.out", "b")));
        let clash = zero(
            "[[node]]
id = \"0000000000000012\"
folder = \"nodes/count\"
             [[wire]]
from = \"0000000000000011.out\"
to = \"0000000000000012.n\"
",
        );
        // Num a is audio, so pick e and the delay f of their cycle are audio too (9.2).
        let audio = |more: &str| {
            let text = format!(
                "format = 1
[[node]]
id = \"000000000000000a\"
folder = \"nodes/num\"
domain = \"audio\"
[[node]]
id = \"000000000000000e\"
                 folder = \"nodes/pick\"
[[node]]
id = \"000000000000000f\"
builtin = \"delay\"
{more}{}{}                 [[wire]]
from = \"000000000000000e.out0\"
to = \"000000000000000f.value\"
",
                wire("0a.out", "a"),
                wire("0f.out", "b")
            );
            check(&root, &toml::from_str(&text).unwrap())
        };
        let (heard_loop, batch_delay) = (audio(""), audio("domain = \"batch\"
"));
        std::fs::create_dir_all(root.join("connections/show")).unwrap();
        let named = check(&root, &graph(""));
        let num_logs = uses_log(&root, "nodes", "num");
        std::fs::remove_dir_all(&root).unwrap();

        let plan = good.unwrap_or_else(|p| panic!("{p:?}"));
        assert_eq!(plan.steps.len(), 2);
        assert_eq!(plan.slots.len(), 1);
        assert_eq!((plan.steps[0].consumers.as_slice(), plan.steps[0].readers.as_slice()), (&[1][..], &[vec![1]][..]));
        assert!(matches!(plan.steps[1].inputs[0], Source::Wire { step: 0, output: 0 }));
        // Only the kind of a held value goes into the plan.
        assert_eq!(plan.kinds.iter().map(|(name, _)| name.as_str()).collect::<Vec<_>>(), ["wide"]);
        assert_eq!(plan.kind(0, 0).map(|(_, kind)| kind.func.as_str()), Some("wide"));
        assert!(plan.log && !num_logs, "`show` has the log crate, `num` does not");
        let named = named.err().unwrap();
        assert!(named.len() == 1 && named[0].starts_with("connections/show: cannot read Cargo.toml"), "{named:?}");
        let bad = bad.err().unwrap();
        assert_eq!(bad.len(), 2, "{bad:?}");
        assert!(bad[0].contains("no such pin") && bad[1].contains("more than one wire"));
        assert!(cycle.err().unwrap()[0].contains("cycle"));
        let frame = frame.unwrap_or_else(|p| panic!("{p:?}"));
        let domains: Vec<Domain> = frame.steps.iter().map(|s| s.domain).chain(frame.tasks.iter().map(|t| t.domain)).collect();
        assert_eq!(domains, [Domain::Frame; 3], "show takes the domain of its input");
        assert!(crossed.err().unwrap()[0].contains("from the frame domain to the batch domain"));
        let mixed = mixed.unwrap_or_else(|p| panic!("{p:?}"));
        assert_eq!(mixed.steps.iter().find(|s| s.id.ends_with('b')).map(|s| s.domain), Some(Domain::Frame), "batch and frame inputs give the frame domain");
        let heard: Vec<Domain> = heard.unwrap_or_else(|p| panic!("{p:?}")).steps.iter().map(|s| s.domain).collect();
        assert_eq!(heard, [Domain::Frame, Domain::Audio], "a frame value goes to the audio domain");
        let resolved = resolved.unwrap_or_else(|p| panic!("{p:?}"));
        let pick = resolved.steps.iter().position(|s| s.id.ends_with('e')).unwrap();
        assert_eq!(resolved.steps[pick].types, ["u8", "Vec<u8>"]);
        assert_eq!(resolved.kind(pick, 0).map(|(name, _)| name.as_str()), Some("byte"), "the kind of the resolved type");
        assert!(two.err().unwrap()[0].contains("the wires give two types: `T` is `u8` and `f64`"));
        assert!(open.err().unwrap()[0].contains("connect a wire that gives the type of T"));
        let looped = looped.unwrap_or_else(|p| panic!("{p:?}"));
        let ids: Vec<&str> = looped.steps.iter().map(|s| &s.id[14..]).collect();
        assert_eq!(ids, ["0a", "0d", "0f", "0e"], "the delay runs before the node of its input");
        assert!(looped.is_delay(2) && looped.steps[2].types == ["f64"] && looped.steps[3].domain == Domain::Frame);
        assert!(untyped.err().unwrap()[0].contains("A cycle of generic nodes gives none"));
        assert!(unknown.err().unwrap()[0].contains("no built-in node \"echo\""));
        assert!(unwired.err().unwrap()[0].contains("connect a wire to the input `value` of the delay"));
        let heard_loop = heard_loop.unwrap_or_else(|p| panic!("{p:?}"));
        assert!(heard_loop.steps.iter().all(|s| s.domain == Domain::Audio), "the delay of an audio cycle is audio");
        assert!(batch_delay.err().unwrap()[0].contains("a delay runs in the frame or the audio domain"));
        let backward = backward.unwrap_or_else(|p| panic!("{p:?}"));
        let step = |id: &str| backward.steps.iter().find(|s| s.id.ends_with(id)).unwrap();
        let types = |id: &str| Some(step(id).types.clone());
        let num = backward.steps.iter().position(|s| s.id.ends_with("0a")).unwrap();
        assert_eq!((&step("11").pins[..], &step("0e").pins[..]), (&[num][..], &[][..]), "num a gives the type of zero, zero gives the type of pick");
        assert_eq!((types("11"), types("0e")), (Some(vec!["f64".to_owned()]), Some(vec!["f64".to_owned(), "Vec<f64>".to_owned()])));
        let clash = clash.err().unwrap();
        assert!(clash.iter().any(|p| p.ends_with("0000000000000011: the wires give two types: `T` is `f64` and `u8`")), "{clash:?}");
        let latest = latest.unwrap_or_else(|p| panic!("{p:?}"));
        let parts: Vec<(Domain, &[usize])> = latest.tasks.iter().map(|t| (t.domain, &t.steps[..])).collect();
        assert_eq!(parts, [(Domain::Batch, &[0][..]), (Domain::Frame, &[1][..])], "a task holds the steps of one domain");
    }

    #[test]
    fn the_log_crate_is_found_in_a_target_table_and_under_another_name() {
        let root = std::env::temp_dir().join(format!("patina-log-{}", std::process::id()));
        let manifests = [
            ("plain", "[dependencies]
log = \"0.4\"
"),
            ("target", "[target.'cfg(windows)'.dependencies]
log = \"0.4\"
"),
            ("renamed", "[dependencies]
logging = { package = \"log\", version = \"0.4\" }
"),
            ("other", "[dependencies]
log = { package = \"not-log\", version = \"1\" }
logos = \"0.15\"
"),
        ];
        for (name, text) in manifests {
            std::fs::create_dir_all(root.join("nodes").join(name)).unwrap();
            std::fs::write(root.join("nodes").join(name).join("Cargo.toml"), text).unwrap();
        }
        let found: Vec<bool> = manifests.iter().map(|(name, _)| uses_log(&root, "nodes", name)).collect();
        std::fs::remove_dir_all(&root).unwrap();
        assert_eq!(found, [true, true, true, false]);
    }

    #[test]
    fn an_unwired_input_needs_parameter_support() {
        let root = std::env::temp_dir().join(format!("patina-param-{}", std::process::id()));
        let files = [
            ("nodes/paint/src/lib.rs", "#[patina_sdk::node]
pub fn paint(f: &frame::Frame, m: Mode, n: Option<f32>, s: &str, x: f32) {}
"),
            ("nodes/paint/Cargo.toml", "[package]
name = \"paint\"
"),
            ("types/frame/src/lib.rs", "pub struct Frame;
"),
            ("types/mode/src/kinds/mod.rs", "impl patina_sdk::Param
    for crate::Mode { }
"),
        ];
        for (path, text) in files {
            std::fs::create_dir_all(root.join(path).parent().unwrap()).unwrap();
            std::fs::write(root.join(path), text).unwrap();
        }
        let graph = toml::from_str("format = 1
[[node]]
id = \"000000000000000a\"
folder = \"nodes/paint\"
").unwrap();
        let problems = check(&root, &graph).err().unwrap_or_default();
        std::fs::remove_dir_all(&root).unwrap();
        let inputs: Vec<&str> = problems.iter().filter_map(|p| p.split('`').nth(1)).collect();
        assert_eq!(inputs, ["f", "n"], "{problems:?}");
        assert!(!impls_param("impl Param for Moded {}", "Mode") && !impls_param("impl MyParam for Mode {}", "Mode") && impls_param("impl<T> Param for Mode<T> {}", "Mode"));
    }
}

//! Checks a graph and puts its nodes in run order (spec 6.4, 9.2).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use crate::node::{self, Builtin, Domain, Kind, Node};
use crate::project::{GraphFile, is_id};
use crate::types;

mod domain;
mod event;

/// A graph that passed all checks, in run order.
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
pub struct Task {
    pub steps: Vec<usize>,
    /// The number of tasks that must end before this task starts.
    pub inputs: u32,
    /// The tasks that wait for this task.
    pub next: Vec<usize>,
    pub domain: Domain,
}

/// One use of a node folder (6.2).
pub struct Step {
    pub id: String,
    /// The index in `Plan::nodes`.
    pub node: usize,
    /// One source for each input of the node, in order.
    pub inputs: Vec<Source>,
    /// The number of wires from each output, in the order of `Node::output_keys`.
    pub consumers: Vec<usize>,
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

pub enum Source {
    Slot(usize),
    Wire { step: usize, output: usize },
}

pub struct Slot {
    pub step: usize,
    /// The key of the input.
    pub key: String,
    pub is_str: bool,
}

/// Crate names that a node folder cannot have, after `-` changes to `_`.
/// Crate names of Rust and Patina, the keywords of Rust, the folder names of Cargo and the device
/// names of Windows. Cargo or Windows refuses a crate or a folder with one of these names.
const RESERVED: &str = "std core alloc proc_macro test patina_sdk patina_macros patina_log patina_event cluster_batch cluster_frame cluster_audio cluster_event \
    as async await break const continue crate dyn else enum extern false fn for gen if impl in let \
    loop match mod move mut pub ref return self static struct super trait true type unsafe use \
    where while abstract become box do final macro override priv try typeof unsized virtual yield \
    build deps examples incremental con prn aux nul com1 com2 com3 com4 com5 com6 com7 com8 com9 \
    lpt1 lpt2 lpt3 lpt4 lpt5 lpt6 lpt7 lpt8 lpt9";

struct Entry {
    id: String,
    node: usize,
    /// The wired source of each input: the entry and the output index.
    wired: Vec<Option<(usize, usize)>>,
    /// The domain that the node declares or that the user selected (9.1).
    domain: Option<Domain>,
}

/// Checks a graph of the project at `root`. Returns all the problems that it finds.
pub fn check(root: &Path, graph: &GraphFile) -> Result<Plan, Vec<String>> {
    let mut problems = Vec::new();
    let kinds = read_kinds(root, &mut problems);
    let mut nodes: Vec<(String, Node)> = Vec::new();
    let mut by_name: HashMap<String, Option<usize>> = HashMap::new();
    let mut entries: Vec<Option<Entry>> = Vec::new();
    let mut by_id: HashMap<&str, usize> = HashMap::new();
    let mut builtins: Vec<(usize, Builtin)> = Vec::new();

    for (index, entry) in graph.node.iter().enumerate() {
        entries.push(None);
        let id = entry.id.as_str();
        let mut fail = |text: String| problems.push(format!("node {id}: {text}"));
        // A node of a subgraph use has the ID path `<use ID>/<node ID>` (6.4).
        if !id.split('/').all(is_id) {
            fail("the ID must be 16 lowercase hexadecimal characters".to_owned());
            continue;
        }
        if by_id.insert(id, index).is_some() {
            fail("two nodes have this ID".to_owned());
            continue;
        }
        if entry.settings.is_some() {
            fail("node settings (4.3) are not ready yet, so remove the settings of the node".to_owned());
        }
        let node_index = match (&entry.folder, &entry.builtin) {
            (None, Some(name)) => match Builtin::parse(name) {
                Some(builtin) => match builtins.iter().find(|(_, b)| *b == builtin) {
                    Some(&(i, _)) => i,
                    None => {
                        nodes.push((builtin.name().to_owned(), builtin.node()));
                        builtins.push((nodes.len() - 1, builtin));
                        nodes.len() - 1
                    }
                },
                None => {
                    fail(format!("Patina has no built-in node {name:?} (spec 9.2)"));
                    continue;
                }
            },
            (Some(_), Some(_)) => {
                fail("give the node a folder or a built-in node, not both".to_owned());
                continue;
            }
            (None, None) => {
                fail("the node has no folder".to_owned());
                continue;
            }
            (Some(folder), None) => {
                let Some(name) = folder.strip_prefix("nodes/") else {
                    fail(format!("the folder {folder:?} is not in nodes/"));
                    continue;
                };
                let slot = by_name.entry(name.to_owned()).or_insert_with(|| match read_node(root, name) {
                    Ok(node) => {
                        nodes.push((name.to_owned(), node));
                        Some(nodes.len() - 1)
                    }
                    Err(text) => {
                        problems.push(format!("{folder}: {text}"));
                        None
                    }
                });
                let Some(node_index) = *slot else { continue };
                node_index
            }
        };
        let folder = entry.folder.as_deref().or(entry.builtin.as_deref()).unwrap_or_default();
        let node = &nodes[node_index].1;
        for key in entry.params.keys() {
            if !node.inputs.iter().any(|i| &i.key == key) {
                problems.push(format!("node {id}: {folder} has no input `{key}` for the parameter"));
            }
        }
        let chosen = entry.domain.as_deref().map(Domain::parse).transpose().unwrap_or_else(|text| {
            problems.push(format!("node {id}: {text}"));
            None
        });
        if let (Some(chosen), Some(declared)) = (chosen, node.domain)
            && chosen != declared
        {
            problems.push(format!("node {id}: {folder} declares the {} domain", declared.name()));
        }
        let wired = vec![None; node.inputs.len()];
        entries[index] = Some(Entry { id: id.to_owned(), node: node_index, wired, domain: node.domain.or(chosen) });
    }

    let is_delay = |node: usize| builtins.contains(&(node, Builtin::Delay));
    let fires = |node: usize| builtins.iter().any(|&(i, b)| i == node && b.fires());
    let mut edges: Vec<(usize, usize)> = Vec::new();
    for wire in &graph.wire {
        let from = find_pin(&wire.from, &by_id, &entries, &nodes, |n| n.output_keys());
        let to = find_pin(&wire.to, &by_id, &entries, &nodes, |n| {
            n.inputs.iter().map(|i| i.key.clone()).collect()
        });
        let ((from_entry, output), (to_entry, input)) = match (from, to) {
            (Ok(from), Ok(to)) => (from, to),
            (from, to) => {
                let ends = [(&wire.from, from.err()), (&wire.to, to.err())];
                for (end, text) in ends.into_iter().filter_map(|(e, t)| Some((e, t?))) {
                    problems.push(format!("wire end {end:?}: {text}"));
                }
                continue;
            }
        };
        let entry = entries[to_entry].as_mut().expect("find_pin gives a checked entry");
        if entry.wired[input].replace((from_entry, output)).is_some() {
            problems.push(format!("the input {:?} has more than one wire", wire.to));
        }
        if fires(entry.node) {
            problems.push(format!("the input {:?} is a setting of the trigger. Set it in its parameter field (spec 9.4)", wire.to));
        }
        // The delay gives the value of the previous frame, so its input wire makes no cycle (9.2).
        if !is_delay(entry.node) {
            edges.push((from_entry, to_entry));
        }
    }
    for order in &graph.order {
        match (by_id.get(order.before.as_str()), by_id.get(order.after.as_str())) {
            (Some(&before), Some(&after)) => edges.push((before, after)),
            _ => problems.push(format!("the order wire {} to {} names no node", order.before, order.after)),
        }
    }
    if !problems.is_empty() {
        return Err(problems);
    }

    let entries: Vec<Entry> = entries.into_iter().map(|e| e.expect("no problems")).collect();
    let order = sort(entries.len(), &edges).map_err(|left| {
        let ids: Vec<&str> = left.iter().map(|&i| entries[i].id.as_str()).collect();
        vec![format!("the graph has a cycle through these nodes: {}", ids.join(", "))]
    })?;
    let mut step_of = vec![0; entries.len()];
    for (step, &entry) in order.iter().enumerate() {
        step_of[entry] = step;
    }
    let mut steps: Vec<Step> = order
        .iter()
        .map(|&e| Step {
            id: entries[e].id.clone(),
            node: entries[e].node,
            inputs: Vec::new(),
            consumers: vec![0; nodes[entries[e].node].1.output_keys().len()],
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

/// Checks the name of a node folder. A name with a path separator or `..` does not pass.
pub fn check_name(name: &str) -> Result<(), String> {
    let valid = name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
    if name.is_empty() || name.len() > 64 || !valid || name.starts_with(|c: char| c.is_ascii_digit()) {
        return Err("a folder name has 1 to 64 letters, digits, _ and -, and starts with a letter".into());
    }
    let key = name_key(name);
    if RESERVED.split_whitespace().any(|r| r == key) {
        return Err(format!("the name {name:?} is reserved. Use another name"));
    }
    Ok(())
}

/// The crate name as Rust and Windows compare it: with no regard to case, and with `-` as `_`.
pub fn name_key(name: &str) -> String {
    name.to_ascii_lowercase().replace('-', "_")
}

pub fn read_node(root: &Path, name: &str) -> Result<Node, String> {
    node::parse(&read_crate(root, "nodes", name)?).map_err(|e| format!("src/lib.rs: {e}"))
}

/// Reads `src/lib.rs` of the crate folder `<group>/<name>`, after the checks of its name.
fn read_crate(root: &Path, group: &str, name: &str) -> Result<String, String> {
    check_name(name)?;
    let dir = root.join(group).join(name);
    let read = |file: &str| {
        std::fs::read_to_string(dir.join(file)).map_err(|e| format!("cannot read {file}: {e}"))
    };
    let manifest: toml::Table = read("Cargo.toml")?.parse().map_err(|e| format!("Cargo.toml: {e}"))?;
    let package = manifest.get("package").and_then(|p| p.get("name")).and_then(|n| n.as_str());
    if package != Some(name) {
        return Err(format!("the package name in Cargo.toml must be {name:?}, the folder name"));
    }
    read("src/lib.rs")
}

/// Finds the entry and the pin index of a wire end `<node ID>.<pin key>`.
fn find_pin(
    end: &str,
    by_id: &HashMap<&str, usize>,
    entries: &[Option<Entry>],
    nodes: &[(String, Node)],
    keys: impl Fn(&Node) -> Vec<String>,
) -> Result<(usize, usize), String> {
    let (id, key) = end.split_once('.').ok_or("write the end as <node ID>.<pin key>")?;
    let &index = by_id.get(id).ok_or("no node has this ID")?;
    let entry = entries[index].as_ref().ok_or("the node has an error")?;
    let (name, node) = &nodes[entry.node];
    let pin = keys(node).iter().position(|k| k == key);
    pin.map(|pin| (index, pin)).ok_or_else(|| format!("nodes/{name} has no such pin"))
}

/// Kahn's algorithm. Ties go in file order. Returns the nodes of a cycle on failure.
fn sort(count: usize, edges: &[(usize, usize)]) -> Result<Vec<usize>, Vec<usize>> {
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

/// True for a `bool`, `char` or number type, also behind `&` or `&mut`.
fn scalar(ty: &str) -> bool {
    const SCALARS: [&str; 16] = ["bool", "char", "f32", "f64", "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize"];
    SCALARS.contains(&ty.trim_start_matches('&').trim_start_matches("mut ").trim())
}

/// True for a scalar type of 64 bits or fewer. Its value can go between tasks without a lock (9.2).
pub fn is_word(ty: &str) -> bool {
    scalar(ty) && !ty.contains("128")
}

/// True when step `n` is so small that a task of its own costs more than the step (9.2): a delay,
/// or a node with only `bool`, `char` and number values, for example a NAND gate. `heavy` stops it.
fn is_light(steps: &[Step], nodes: &[(String, Node)], n: usize, is_delay: bool) -> bool {
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
fn split(steps: &mut [Step], edges: &[(usize, usize)], trigger: &[Option<usize>], light: &[bool]) -> Vec<Task> {
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

    #[test]
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
        let good = check(&root, &graph("[[wire]]\nfrom = \"000000000000000a.out\"\nto = \"000000000000000b.v\"\n"));
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
        assert_eq!(plan.steps[0].consumers, [1]);
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
}

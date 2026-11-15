//! Checks a graph and puts its nodes in run order (spec 6.4, 9.2).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use crate::node::{self, Node};
use crate::project::{GraphFile, is_id};

/// A graph that passed all checks, in run order.
pub struct Plan {
    /// The node folders that the graph uses, by folder name. The name is the crate name (6.2).
    pub nodes: Vec<(String, Node)>,
    pub steps: Vec<Step>,
    /// One parameter slot for each input with no wire.
    pub slots: Vec<Slot>,
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
}

pub enum Source {
    Slot(usize),
    Wire { step: usize, output: usize },
}

pub struct Slot {
    pub step: usize,
    /// The key of the input.
    pub key: String,
    /// The text form from the graph file, or `None` for the default value (4.8).
    pub text: Option<String>,
    pub is_str: bool,
}

/// Crate names that a node folder cannot have, after `-` changes to `_`.
const RESERVED: [&str; 8] =
    ["std", "core", "alloc", "proc_macro", "test", "patina_sdk", "patina_macros", "cluster_main"];

struct Entry {
    id: String,
    node: usize,
    /// The wired source of each input: the entry and the output index.
    wired: Vec<Option<(usize, usize)>>,
}

/// Checks a graph of the project at `root`. Returns all the problems that it finds.
pub fn check(root: &Path, graph: &GraphFile) -> Result<Plan, Vec<String>> {
    let mut problems = Vec::new();
    let mut nodes: Vec<(String, Node)> = Vec::new();
    let mut by_name: HashMap<String, Option<usize>> = HashMap::new();
    let mut entries: Vec<Option<Entry>> = Vec::new();
    let mut by_id: HashMap<&str, usize> = HashMap::new();

    for (index, entry) in graph.node.iter().enumerate() {
        entries.push(None);
        let id = entry.id.as_str();
        let mut fail = |text: String| problems.push(format!("node {id}: {text}"));
        if !is_id(id) {
            fail("the ID must be 16 lowercase hexadecimal characters".to_owned());
            continue;
        }
        if by_id.insert(id, index).is_some() {
            fail("two nodes have this ID".to_owned());
            continue;
        }
        if entry.subgraph.is_some() {
            fail("subgraphs come at milestone 7".to_owned());
            continue;
        }
        if entry.settings.is_some() {
            fail("node settings (4.3) come at milestone 5".to_owned());
        }
        let Some(folder) = &entry.folder else {
            fail("the node has no folder".to_owned());
            continue;
        };
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
        let node = &nodes[node_index].1;
        for key in entry.params.keys() {
            if !node.inputs.iter().any(|i| &i.key == key) {
                problems.push(format!("node {id}: {folder} has no input `{key}` for the parameter"));
            }
        }
        let wired = vec![None; node.inputs.len()];
        entries[index] = Some(Entry { id: id.to_owned(), node: node_index, wired });
    }

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
        edges.push((from_entry, to_entry));
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
                None => {
                    let text = graph.node[e].params.get(&input.key).cloned();
                    slots.push(Slot { step, key: input.key.clone(), text, is_str: input.is_str });
                    Source::Slot(slots.len() - 1)
                }
            };
            steps[step].inputs.push(source);
        }
    }
    Ok(Plan { nodes, steps, slots })
}

fn read_node(root: &Path, name: &str) -> Result<Node, String> {
    let valid = name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
    if name.is_empty() || !valid || name.starts_with(|c: char| c.is_ascii_digit()) {
        return Err("a node folder name has only letters, digits, _ and -, and starts with a letter".into());
    }
    if RESERVED.contains(&name.replace('-', "_").as_str()) {
        return Err(format!("the name {name:?} is reserved. Rename the folder"));
    }
    let dir = root.join("nodes").join(name);
    let read = |file: &str| {
        std::fs::read_to_string(dir.join(file)).map_err(|e| format!("cannot read {file}: {e}"))
    };
    let manifest: toml::Table = read("Cargo.toml")?.parse().map_err(|e| format!("Cargo.toml: {e}"))?;
    let package = manifest.get("package").and_then(|p| p.get("name")).and_then(|n| n.as_str());
    if package != Some(name) {
        return Err(format!("the package name in Cargo.toml must be {name:?}, the folder name"));
    }
    node::parse(&read("src/lib.rs")?).map_err(|e| format!("src/lib.rs: {e}"))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sort_keeps_file_order_and_finds_cycles() {
        assert_eq!(sort(3, &[(2, 0)]), Ok(vec![1, 2, 0]));
        assert_eq!(sort(3, &[(0, 1), (1, 0)]), Err(vec![0, 1]));
    }

    #[test]
    fn graph_problems_are_all_reported() {
        let root = std::env::temp_dir().join(format!("patina-plan-{}", std::process::id()));
        for (name, code) in [("num", "pub fn num(v: f64) -> f64 { v }"), ("show", "pub fn show(v: &f64) {}")] {
            let dir = root.join("nodes").join(name);
            std::fs::create_dir_all(dir.join("src")).unwrap();
            std::fs::write(dir.join("Cargo.toml"), format!("[package]\nname = \"{name}\"\n")).unwrap();
            std::fs::write(dir.join("src/lib.rs"), format!("#[patina_sdk::node]\n{code}")).unwrap();
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
        std::fs::remove_dir_all(&root).unwrap();

        let plan = good.unwrap_or_else(|p| panic!("{p:?}"));
        assert_eq!(plan.steps.len(), 2);
        assert_eq!(plan.slots.len(), 1);
        assert_eq!(plan.steps[0].consumers, [1]);
        assert!(matches!(plan.steps[1].inputs[0], Source::Wire { step: 0, output: 0 }));
        let bad = bad.err().unwrap();
        assert_eq!(bad.len(), 2, "{bad:?}");
        assert!(bad[0].contains("no such pin") && bad[1].contains("more than one wire"));
        assert!(cycle.err().unwrap()[0].contains("cycle"));
    }
}

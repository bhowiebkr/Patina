//! Material instances flattened into one graph for the engine: each instance's material is
//! copied in (its Output left out) under fresh ids from `INNER` up, the instance's overrides
//! are applied to the copies, and the copied result feeds the instance's hidden `material`
//! input. Nested instances flatten the same way.

use std::collections::{BTreeMap, VecDeque};

use forge_nodes::{Graph, Link, NodeId};

/// Node ids from here up are copies from instanced materials.
pub const INNER: u32 = 1 << 24;
pub const INSTANCE: &str = "material_instance";
/// Instances nested deeper than this are reported instead of copied.
const DEPTH: usize = 8;

pub struct Flat {
    pub graph: Graph,
    /// Each copied node's material and its id there.
    pub origins: BTreeMap<NodeId, (String, NodeId)>,
    pub errors: Vec<String>,
    pub aliases: OutputAliases,
}

pub type OutputAliases = BTreeMap<(NodeId, String), (NodeId, String)>;

pub fn resolve(aliases: &OutputAliases, node: NodeId, output: &str) -> Option<(NodeId, String)> {
    let mut value = (node, output.to_owned());
    let mut seen = std::collections::BTreeSet::new();
    while let Some(next) = aliases.get(&value) {
        if !seen.insert(value) {
            return None;
        }
        value = next.clone();
    }
    Some(value)
}

fn alias(flat: &mut Flat, from: NodeId, output: &str, target: (NodeId, String)) {
    for link in &mut flat.graph.links {
        if link.from == from && link.output == output {
            link.from = target.0;
            link.output = target.1.clone();
        }
    }
    flat.aliases.insert((from, output.into()), target);
}

/// An instance's override key for its material's node `node`, parameter `param`.
pub fn key(node: NodeId, param: &str) -> String {
    format!("{}/{param}", node.0)
}

/// Flattens material `id`'s `graph`; `lookup` finds the instanced materials.
pub fn flatten<'g>(id: &str, graph: &Graph, lookup: &dyn Fn(&str) -> Option<&'g Graph>) -> Flat {
    flatten_impl(None, id, graph, lookup)
}

pub fn flatten_with_registry<'g>(
    registry: &forge_nodes::Registry,
    id: &str,
    graph: &Graph,
    lookup: &dyn Fn(&str) -> Option<&'g Graph>,
) -> Flat {
    flatten_impl(Some(registry), id, graph, lookup)
}

fn flatten_impl<'g>(
    registry: Option<&forge_nodes::Registry>,
    id: &str,
    graph: &Graph,
    lookup: &dyn Fn(&str) -> Option<&'g Graph>,
) -> Flat {
    let mut flat = Flat {
        graph: graph.clone(),
        origins: BTreeMap::new(),
        errors: Vec::new(),
        aliases: BTreeMap::new(),
    };
    let mut next = INNER;
    let mut conversions = std::collections::BTreeSet::new();
    // Instance nodes to expand, each with the materials it sits inside, outermost first.
    let mut queue: VecDeque<(NodeId, Vec<String>)> = graph
        .nodes
        .iter()
        .filter(|n| forge_nodes::subgraph::reference(n).is_some())
        .map(|n| (n.id, vec![id.to_owned()]))
        .collect();
    while let Some((at, chain)) = queue.pop_front() {
        let Some(node) = flat.graph.node(at) else {
            continue;
        };
        let reference = forge_nodes::subgraph::reference(node)
            .unwrap_or("")
            .to_owned();
        if reference.is_empty() {
            continue;
        }
        let inside = chain.last().map_or("", String::as_str);
        if chain.contains(&reference) {
            flat.errors
                .push(format!("{inside}: instancing {reference} makes a loop"));
            continue;
        }
        if chain.len() > DEPTH {
            flat.errors
                .push(format!("{inside}: instances nest deeper than {DEPTH}"));
            continue;
        }
        let Some(inner) = lookup(&reference) else {
            flat.errors
                .push(format!("{inside}: no material \"{reference}\""));
            continue;
        };
        if flat.graph.nodes.len().saturating_add(inner.nodes.len()) > 100_000 {
            flat.errors
                .push(format!("{reference}: expanded graph exceeds 100000 nodes"));
            break;
        }
        if let Some(interface) = &inner.interface {
            if node.kind != format!("{}{reference}", forge_nodes::subgraph::PREFIX) {
                flat.errors
                    .push(format!("{reference}: use its declared subgraph node"));
                continue;
            }
            if let Err(error) = interface.validate(inner) {
                flat.errors.push(format!("{reference}: {error}"));
                continue;
            }
        } else if node.kind != INSTANCE {
            flat.errors
                .push(format!("{reference}: missing subgraph interface"));
            continue;
        }
        let incoming: Vec<_> = inner
            .interface
            .as_ref()
            .into_iter()
            .flat_map(|i| &i.inputs)
            .filter_map(|p| {
                flat.graph
                    .source(at, &p.id)
                    .map(|(from, output)| (p.node, from, output.to_owned()))
            })
            .collect();
        let (params, text) = (node.params.clone(), node.text.clone());
        let mut ids = BTreeMap::new();
        for n in inner.nodes.iter().filter(|n| n.kind != "output") {
            let copy_id = NodeId(next);
            next += 1;
            ids.insert(n.id, copy_id);
            let mut copy = n.clone();
            copy.id = copy_id;
            let prefix = format!("{}/", n.id.0);
            for (k, v) in &params {
                if let Some(param) = k.strip_prefix(&prefix) {
                    copy.params.insert(param.to_owned(), v.clone());
                }
            }
            for (k, v) in &text {
                if let Some(param) = k.strip_prefix(&prefix) {
                    copy.text.insert(param.to_owned(), v.clone());
                }
            }
            if forge_nodes::subgraph::reference(&copy).is_some() {
                let mut chain = chain.clone();
                chain.push(reference.clone());
                queue.push_back((copy_id, chain));
            }
            flat.origins.insert(copy_id, (reference.clone(), n.id));
            flat.graph.nodes.push(copy);
        }
        for l in &inner.links {
            if let (Some(from), Some(to)) = (ids.get(&l.from), ids.get(&l.to)) {
                flat.graph.links.push(Link {
                    from: *from,
                    output: l.output.clone(),
                    to: *to,
                    input: l.input.clone(),
                });
            }
        }
        if let Some(interface) = &inner.interface {
            // Boundaries are wire aliases, never extra sampling/evaluation passes.
            for port in &interface.inputs {
                let Some(copy) = ids.get(&port.node).copied() else {
                    continue;
                };
                let target = incoming
                    .iter()
                    .find(|(node, _, _)| *node == port.node)
                    .map(|(_, from, output)| (*from, output.clone()))
                    .or_else(|| {
                        flat.graph
                            .source(copy, "source")
                            .map(|(n, o)| (n, o.to_owned()))
                    });
                if let Some(target) = target {
                    if output_socket(registry, &flat, &target) == Some(port.socket) {
                        alias(&mut flat, copy, "output", target);
                        flat.graph.remove(copy);
                    } else {
                        flat.graph.disconnect(copy, "source");
                        flat.graph.links.push(Link {
                            from: target.0,
                            output: target.1,
                            to: copy,
                            input: "source".into(),
                        });
                        conversions.insert(copy);
                    }
                }
            }
            for p in &interface.outputs {
                if let Some(copy) = ids.get(&p.node).copied() {
                    let source = flat
                        .graph
                        .source(copy, "source")
                        .map(|(n, o)| (n, o.to_owned()));
                    let target = if let Some(source) = source
                        .filter(|source| output_socket(registry, &flat, source) == Some(p.socket))
                    {
                        alias(&mut flat, copy, "output", source.clone());
                        flat.graph.remove(copy);
                        source
                    } else {
                        conversions.insert(copy);
                        (copy, "output".into())
                    };
                    alias(&mut flat, at, &p.id, target);
                }
            }
            flat.graph.remove(at);
            continue;
        }
        let result = inner
            .output()
            .and_then(|out| inner.source(out, "material"))
            .and_then(|(from, output)| Some((*ids.get(&from)?, output.to_owned())));
        if let Some((from, output)) = result {
            flat.graph.disconnect(at, "source");
            flat.graph.links.push(Link {
                from,
                output,
                to: at,
                input: "source".to_owned(),
            });
        }
    }
    for node in &conversions {
        let mut source = *node;
        let mut seen = std::collections::BTreeSet::new();
        while conversions.contains(&source) && seen.insert(source) {
            let Some((next, _)) = flat.graph.source(source, "source") else {
                break;
            };
            source = next;
        }
        if let Some(size) = flat.graph.node(source).map(|n| n.size)
            && let Some(node) = flat.graph.node_mut(*node)
        {
            node.size = size;
        }
    }
    flat
}

fn output_socket(
    registry: Option<&forge_nodes::Registry>,
    flat: &Flat,
    target: &(NodeId, String),
) -> Option<forge_nodes::Socket> {
    let (node, output) = resolve(&flat.aliases, target.0, &target.1)?;
    registry?
        .get(&flat.graph.node(node)?.kind)?
        .outputs
        .iter()
        .find(|p| p.id == output)
        .map(|p| p.socket)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link(graph: &mut Graph, from: NodeId, output: &str, to: NodeId, input: &str) {
        graph.links.push(Link {
            from,
            output: output.to_owned(),
            to,
            input: input.to_owned(),
        });
    }

    /// `paint` -> output, with paint's colour exposed.
    fn paint() -> Graph {
        let mut g = Graph::default();
        let fill = g.add("noise", [0.0; 2]);
        let out = g.add("output", [0.0; 2]);
        link(&mut g, fill, "material", out, "material");
        g
    }

    /// One instance of `reference` -> output.
    fn instancing(reference: &str) -> Graph {
        let mut g = Graph::default();
        let inst = g.add(INSTANCE, [0.0; 2]);
        let out = g.add("output", [0.0; 2]);
        if let Some(n) = g.node_mut(inst) {
            n.text.insert("reference".into(), reference.into());
            n.params
                .insert(key(NodeId(1), "color"), vec![1.0, 0.0, 0.0]);
        }
        link(&mut g, inst, "material", out, "material");
        g
    }

    #[test]
    fn copies_the_material_and_applies_overrides() {
        let paint = paint();
        let outer = instancing("paint");
        let flat = flatten("tank", &outer, &|id| (id == "paint").then_some(&paint));
        assert!(flat.errors.is_empty(), "{:?}", flat.errors);
        let copy = NodeId(INNER);
        let node = flat
            .graph
            .node(copy)
            .map(|n| (n.kind.as_str(), n.params.get("color")));
        assert_eq!(node, Some(("noise", Some(&vec![1.0, 0.0, 0.0]))));
        assert_eq!(
            flat.graph.source(NodeId(1), "source"),
            Some((copy, "material"))
        );
        assert_eq!(
            flat.origins.get(&copy),
            Some(&("paint".to_owned(), NodeId(1)))
        );
        assert_eq!(flat.graph.nodes.len(), 3, "the inner output is left out");
    }

    #[test]
    fn nests_and_reports_loops_and_missing_materials() {
        let paint = paint();
        let middle = instancing("paint");
        let looped = instancing("tank");
        let lookup = |id: &str| match id {
            "paint" => Some(&paint),
            "middle" => Some(&middle),
            "tank" => Some(&looped),
            _ => None,
        };
        let flat = flatten("tank", &instancing("middle"), &lookup);
        assert!(flat.errors.is_empty(), "{:?}", flat.errors);
        assert_eq!(flat.graph.nodes.len(), 4);
        assert_eq!(
            flat.origins.get(&NodeId(INNER + 1)).map(|o| o.0.as_str()),
            Some("paint")
        );

        let flat = flatten("tank", &instancing("tank"), &lookup);
        assert_eq!(flat.errors.len(), 1);
        let flat = flatten("tank", &instancing("gone"), &lookup);
        assert_eq!(flat.errors, ["tank: no material \"gone\""]);
    }
}

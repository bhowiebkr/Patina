//! Transactional extraction and editing of reusable graphs. The legacy material flattener is retained.
use super::*;
use forge_nodes::{
    Link, NodeId, Socket,
    subgraph::{Interface, PREFIX, Port, port_kind},
};
use std::{collections::BTreeSet, sync::Arc};

pub(crate) fn validate_documents(
    registry: &Registry,
    root: &Graph,
    materials: &BTreeMap<String, Graph>,
) -> Result<(), String> {
    validate_graph(registry, root)?;
    for (id, graph) in materials {
        super::validate_links(registry, graph).map_err(|e| format!("{id}: {e}"))?;
        let Some(interface) = &graph.interface else {
            continue;
        };
        interface
            .validate(graph)
            .map_err(|e| format!("{id}: {e}"))?;
        let kind = format!("{PREFIX}{id}");
        let def = registry
            .get(&kind)
            .ok_or_else(|| format!("{id}: missing pinned interface definition"))?;
        let inputs: Vec<_> = def.inputs.iter().filter(|p| !p.hidden).collect();
        if inputs.len() != interface.inputs.len()
            || def.outputs.len() != interface.outputs.len()
            || inputs
                .iter()
                .zip(&interface.inputs)
                .any(|(a, b)| a.id != b.id || a.socket != b.socket)
            || def
                .outputs
                .iter()
                .zip(&interface.outputs)
                .any(|(a, b)| a.id != b.id || a.socket != b.socket)
        {
            return Err(format!(
                "{id}: declared ports do not match the pinned subgraph definition"
            ));
        }
    }
    Ok(())
}

impl Session {
    pub fn create_subgraph(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        owner: Option<&str>,
        selection: &BTreeSet<NodeId>,
    ) -> Result<NodeId, String> {
        let mut state = self.snapshot();
        let parent = match owner {
            Some(id) => state
                .materials
                .get(id)
                .ok_or("Subgraph is no longer available")?,
            None => &state.graph,
        };
        let id = (1..)
            .map(|n| format!("graph_{n}"))
            .find(|id| {
                !state.materials.contains_key(id)
                    && self
                        .engine
                        .registry()
                        .get(&format!("{PREFIX}{id}"))
                        .is_none()
            })
            .ok_or("No available subgraph identity")?;
        let (parent, child, instance) = extract(parent, self.engine.registry(), selection, &id)?;
        if let Some(owner) = owner {
            state.materials.insert(owner.into(), parent);
        } else {
            state.graph = parent;
        }
        state.materials.insert(id, child);
        self.install_subgraphs(device, queue, state)?;
        Ok(instance)
    }

    pub fn update_subgraph(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        id: &str,
        graph: Graph,
    ) -> Result<(), String> {
        let interface = graph
            .interface
            .as_ref()
            .ok_or("Missing subgraph interface")?;
        interface.validate(&graph)?;
        let mut state = self.snapshot();
        let old = state
            .materials
            .get(id)
            .and_then(|g| g.interface.as_ref())
            .ok_or("Subgraph is no longer available")?;
        let kind = format!("{PREFIX}{id}");
        let removed_inputs: BTreeSet<_> = old
            .inputs
            .iter()
            .filter(|p| !interface.inputs.iter().any(|n| n.id == p.id))
            .map(|p| p.id.clone())
            .collect();
        let removed_outputs: BTreeSet<_> = old
            .outputs
            .iter()
            .filter(|p| !interface.outputs.iter().any(|n| n.id == p.id))
            .map(|p| p.id.clone())
            .collect();
        state.materials.insert(id.into(), graph);
        for graph in std::iter::once(&mut state.graph).chain(state.materials.values_mut()) {
            let instances: BTreeSet<_> = graph
                .nodes
                .iter()
                .filter(|n| n.kind == kind)
                .map(|n| n.id)
                .collect();
            graph.links.retain(|l| {
                !(instances.contains(&l.to) && removed_inputs.contains(&l.input)
                    || instances.contains(&l.from) && removed_outputs.contains(&l.output))
            });
        }
        self.install_subgraphs(device, queue, state)
    }

    fn install_subgraphs(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        mut state: DocumentState,
    ) -> Result<(), String> {
        let mut files = self.engine.registry().files().clone();
        let mut linked = self.engine.registry().linked_sources().clone();
        for (id, graph) in &state.materials {
            let Some(interface) = &graph.interface else {
                continue;
            };
            interface.validate(graph)?;
            let kind = format!("{PREFIX}{id}");
            let path = format!("nodes/subgraphs/{kind}.wesl");
            let source = interface.source(&graph.name);
            if files.get(&path) != Some(&source) {
                if state.drafts.contains_key(&path) {
                    return Err(format!(
                        "Resolve the unfinished draft for {path} before changing its interface"
                    ));
                }
                linked.retain(|key, _| !key.starts_with(&format!("{kind}\n")));
                files.insert(path.clone(), source);
            }
            if !state.custom_nodes.contains(&path) {
                state.custom_nodes.push(path);
            }
        }
        // Older material PNGs need these exact helpers only when the user creates a subgraph.
        for (path, source) in Registry::embedded().files() {
            if path
                .rsplit('/')
                .next()
                .is_some_and(|p| p.starts_with("graph_port_"))
            {
                files.entry(path.clone()).or_insert_with(|| source.clone());
            }
        }
        state.sources = Arc::new(SourceSet { files, linked });
        self.restore(device, queue, state)
    }
}

/// Adds a typed boundary. The normal canvas wires its source and consumers.
pub fn add_port(graph: &mut Graph, input: bool, socket: Socket) -> Result<(), String> {
    let interface = graph.interface.as_ref().ok_or("Not a reusable graph")?;
    if interface.inputs.len() + interface.outputs.len() >= 16 {
        return Err("The subgraph already has 16 interface ports".into());
    }
    let prefix = if input { "input" } else { "output" };
    let n = (1..)
        .find(|n| {
            !interface
                .inputs
                .iter()
                .chain(&interface.outputs)
                .any(|p| p.id == format!("{prefix}_{n}"))
        })
        .ok_or("No port identity")?;
    let label = format!("{} {n}", if input { "Input" } else { "Output" });
    let pos = if input {
        [-200., (n - 1) as f32 * 220.]
    } else {
        [600., (n - 1) as f32 * 220.]
    };
    let node = graph.add(port_kind(socket), pos);
    if let Some(node) = graph.node_mut(node) {
        node.title = label.clone();
    }
    let port = Port {
        id: format!("{prefix}_{n}"),
        label,
        socket,
        node,
    };
    let interface = graph.interface.as_mut().ok_or("Missing interface")?;
    if input {
        interface.inputs.push(port);
    } else {
        interface.outputs.push(port);
    }
    Ok(())
}

pub(crate) fn extract(
    parent: &Graph,
    registry: &Registry,
    selected: &BTreeSet<NodeId>,
    reference: &str,
) -> Result<(Graph, Graph, NodeId), String> {
    let selected: BTreeSet<_> = selected
        .iter()
        .copied()
        .filter(|id| {
            parent.node(*id).is_some_and(|n| {
                !registry.get(&n.kind).is_some_and(|d| d.is_output())
                    && !parent.interface.as_ref().is_some_and(|i| i.contains(*id))
            })
        })
        .collect();
    if selected.is_empty() {
        return Err("Select at least one ordinary node to create a reusable graph".into());
    }
    let mut child = parent.clone();
    child.name = format!("Subgraph {}", reference.trim_start_matches("graph_"));
    child.interface = Some(Interface::default());
    child.nodes.retain(|n| selected.contains(&n.id));
    child
        .links
        .retain(|l| selected.contains(&l.from) && selected.contains(&l.to));
    child.exposed.retain(|e| selected.contains(&e.node));
    for preset in &mut child.presets {
        preset.values.retain(|key, _| {
            key.split_once('/')
                .and_then(|(n, _)| n.parse::<u32>().ok())
                .is_some_and(|n| selected.contains(&NodeId(n)))
        });
    }
    child.presets.retain(|p| !p.values.is_empty());
    child
        .canvas
        .frames
        .retain(|f| !f.nodes.is_empty() && f.nodes.iter().all(|n| selected.contains(n)));
    let notes: BTreeSet<_> = child
        .canvas
        .frames
        .iter()
        .flat_map(|f| f.notes.iter().copied())
        .collect();
    child.canvas.notes.retain(|n| notes.contains(&n.id));
    let min_x = child
        .nodes
        .iter()
        .map(|n| n.pos[0])
        .fold(f32::INFINITY, f32::min);
    let min_y = child
        .nodes
        .iter()
        .map(|n| n.pos[1])
        .fold(f32::INFINITY, f32::min);
    let max_x = child
        .nodes
        .iter()
        .map(|n| n.pos[0])
        .fold(f32::NEG_INFINITY, f32::max);
    let mut incoming: BTreeMap<(NodeId, String), Vec<Link>> = BTreeMap::new();
    let mut outgoing: BTreeMap<(NodeId, String), Vec<Link>> = BTreeMap::new();
    for link in &parent.links {
        if !selected.contains(&link.from) && selected.contains(&link.to) {
            incoming
                .entry((link.from, link.output.clone()))
                .or_default()
                .push(link.clone());
        } else if selected.contains(&link.from) && !selected.contains(&link.to) {
            outgoing
                .entry((link.from, link.output.clone()))
                .or_default()
                .push(link.clone());
        }
    }
    if outgoing.is_empty() {
        let node = child.nodes.last().ok_or("Empty subgraph")?;
        let def = registry
            .get(&node.kind)
            .ok_or("Missing selected node definition")?;
        for output in &def.outputs {
            outgoing.entry((node.id, output.id.clone())).or_default();
        }
    }
    if incoming.len() + outgoing.len() > 16 {
        return Err(
            "Selection has more than 16 boundary ports; select a smaller reusable graph".into(),
        );
    }
    let mut outer = parent.clone();
    let moved_frames: BTreeSet<_> = child.canvas.frames.iter().map(|f| f.id).collect();
    for frame in &mut child.canvas.frames {
        if frame.parent.is_some_and(|p| !moved_frames.contains(&p)) {
            frame.parent = None;
        }
    }
    outer
        .canvas
        .frames
        .retain(|f| !moved_frames.contains(&f.id));
    outer.canvas.notes.retain(|n| !notes.contains(&n.id));
    for frame in &mut outer.canvas.frames {
        if frame.parent.is_some_and(|p| moved_frames.contains(&p)) {
            frame.parent = None;
        }
    }
    for id in &selected {
        outer.remove(*id);
    }
    let instance = outer.add(&format!("{PREFIX}{reference}"), [min_x, min_y]);
    for (input, boundaries) in [(true, incoming), (false, outgoing)] {
        for (index, ((source, output), links)) in boundaries.into_iter().enumerate() {
            let source_node = parent.node(source).ok_or("Missing boundary source")?;
            let output_def = registry
                .get(&source_node.kind)
                .and_then(|d| d.outputs.iter().find(|o| o.id == output))
                .ok_or("Missing boundary output")?;
            let socket = output_def.socket;
            let node = child.add(
                port_kind(socket),
                [
                    if input { min_x - 220. } else { max_x + 240. },
                    min_y + index as f32 * 220.,
                ],
            );
            let label = format!("{} / {}", source_node.label(registry), output_def.label);
            if let Some(node) = child.node_mut(node) {
                node.title = label.clone();
            }
            let port = Port {
                id: format!("{}_{index}", if input { "input" } else { "output" }),
                label,
                socket,
                node,
            };
            if input {
                for mut link in links {
                    link.from = node;
                    link.output = "output".into();
                    child.links.push(link);
                }
                outer.links.push(Link {
                    from: source,
                    output,
                    to: instance,
                    input: port.id.clone(),
                });
            } else {
                child.links.push(Link {
                    from: source,
                    output,
                    to: node,
                    input: "source".into(),
                });
                for mut link in links {
                    link.from = instance;
                    link.output = port.id.clone();
                    outer.links.push(link);
                }
            }
            let interface = child.interface.as_mut().ok_or("Missing interface")?;
            if input {
                interface.inputs.push(port);
            } else {
                interface.outputs.push(port);
            }
        }
    }
    child
        .interface
        .as_ref()
        .ok_or("Missing interface")?
        .validate(&child)?;
    Ok((outer, child, instance))
}

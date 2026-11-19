//! Explicitly organize imported source groups using the existing typed subgraph boundaries.
//! This is an import operation, never an implicit migration of a pinned material on load.
use forge_nodes::{Graph, Registry, subgraph::PREFIX};
use std::collections::{BTreeMap, BTreeSet};

/// Organize a newly imported document before its first GPU evaluation.
pub fn apply(state: &mut crate::DocumentState) -> Result<Vec<String>, String> {
    let registry = Registry::from_files(state.sources.files.clone());
    let organized = source_groups(&state.graph, &registry)?;
    if organized
        .materials
        .keys()
        .any(|id| state.materials.contains_key(id))
    {
        return Err("Source subgraph identity already exists".into());
    }
    let mut sources = (*state.sources).clone();
    sources.files.extend(organized.sources.clone());
    for path in organized.sources.keys() {
        if !state.custom_nodes.contains(path) {
            state.custom_nodes.push(path.clone());
        }
    }
    state.graph = organized.graph;
    state.materials.extend(organized.materials);
    state.sources = std::sync::Arc::new(sources);
    Ok(organized.skipped)
}

pub struct Organized {
    pub graph: Graph,
    pub materials: BTreeMap<String, Graph>,
    /// Generated declarations only; extend the existing pinned sources with these.
    pub sources: BTreeMap<String, String>,
    pub skipped: Vec<String>,
}

/// Extract leaf groups before their parents. Source names, colors, hierarchy and shader
/// parameters survive; interface proxies flatten to wire aliases, without extra GPU passes.
pub fn source_groups(graph: &Graph, registry: &Registry) -> Result<Organized, String> {
    graph
        .canvas
        .validate(&graph.nodes.iter().map(|n| n.id).collect())?;
    let mut result = Organized {
        graph: graph.clone(),
        materials: BTreeMap::new(),
        sources: BTreeMap::new(),
        skipped: vec![],
    };
    let mut registry = Registry::from_files(registry.files().clone());
    let mut pending: BTreeSet<_> = graph.canvas.frames.iter().map(|f| f.id).collect();
    while !pending.is_empty() {
        let ready: Vec<_> = result
            .graph
            .canvas
            .frames
            .iter()
            .filter(|f| {
                pending.contains(&f.id)
                    && !result
                        .graph
                        .canvas
                        .frames
                        .iter()
                        .any(|c| c.parent == Some(f.id) && pending.contains(&c.id))
            })
            .map(|f| f.id)
            .collect();
        if ready.is_empty() {
            return Err("Source groups contain a hierarchy loop".into());
        }
        for id in ready {
            pending.remove(&id);
            let frame = result
                .graph
                .canvas
                .frames
                .iter()
                .find(|f| f.id == id)
                .ok_or("Missing source group")?
                .clone();
            let selected: BTreeSet<_> =
                result
                    .graph
                    .nodes
                    .iter()
                    .filter(|n| {
                        result.graph.canvas.frames.iter().any(|f| {
                            result.graph.canvas.inside(f.id, id) && f.nodes.contains(&n.id)
                        })
                    })
                    .map(|n| n.id)
                    .collect();
            if selected.is_empty() {
                continue;
            }
            let reference = (id..)
                .map(|n| format!("source_{n}"))
                .find(|name| registry.get(&format!("{PREFIX}{name}")).is_none())
                .ok_or("No available source group identity")?;
            let (mut parent, mut child, instance) =
                match super::subgraphs::extract(&result.graph, &registry, &selected, &reference) {
                    Ok(value) => value,
                    Err(error) => {
                        result.skipped.push(format!("{}: {error}", frame.title));
                        continue;
                    }
                };
            child.name.clone_from(&frame.title);
            child.canvas.source = Some(serde_json::json!({
                "kind":"source-group", "title":frame.title,
                "description":frame.description, "color":frame.color,
                "source_color_tag":frame.source_color_tag,
                "source_collapsed":frame.folded,
                "authored_canvas_available":false,
            }));
            if let Some(own_frame) = child.canvas.frames.iter_mut().find(|f| f.id == id) {
                own_frame.folded = false;
            }
            // Leave the source frame around its contents inside the child, and make its
            // instance a member of the enclosing source group for the next extraction.
            if let Some(container) = frame
                .parent
                .and_then(|id| parent.canvas.frames.iter_mut().find(|f| f.id == id))
            {
                container.nodes.push(instance);
            }
            if let Some(node) = parent.node_mut(instance) {
                node.title.clone_from(&child.name);
            }
            let path = format!("nodes/subgraphs/{PREFIX}{reference}.wesl");
            let source = child
                .interface
                .as_ref()
                .ok_or("Missing source interface")?
                .source(&child.name);
            result.sources.insert(path.clone(), source.clone());
            let mut files = registry.files().clone();
            files.insert(path, source);
            registry = Registry::from_files(files);
            result.materials.insert(reference, child);
            result.graph = parent;
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_nodes::{Frame, NodeId};

    fn frame(id: u32, name: &str, parent: Option<u32>, nodes: Vec<NodeId>) -> Frame {
        Frame {
            id,
            title: name.into(),
            parent,
            nodes,
            description: "Original source layer".into(),
            color: Some([25, 100, 70, 255]),
            source_color_tag: Some(3),
            pos: [0., 0.],
            size: [500., 500.],
            notes: vec![],
            folded: false,
        }
    }

    #[test]
    fn nested_source_groups_preserve_operations_and_actual_boundary_wires() -> Result<(), String> {
        let registry = Registry::embedded();
        let mut original = Graph::default();
        let base = original.add("noise", [0., 0.]);
        let levels = original.add("combine", [200., 0.]);
        let material = original.add("material", [400., 0.]);
        let output = original.add("output", [600., 0.]);
        original.connect(&registry, base, "output", levels, "a")?;
        original.connect(&registry, levels, "output", material, "roughness")?;
        original.connect(&registry, material, "material", output, "material")?;
        original.canvas.frames = vec![
            frame(1, "Noise", Some(2), vec![base, levels]),
            frame(2, "Base Metal", None, vec![material]),
        ];
        let organized = source_groups(&original, &registry)?;
        assert!(organized.skipped.is_empty());
        assert_eq!(organized.graph.nodes.len(), 2);
        assert_eq!(organized.materials.len(), 2);
        let mut files = registry.files().clone();
        files.extend(organized.sources);
        let registry = Registry::from_files(files);
        super::super::subgraphs::validate_documents(
            &registry,
            &organized.graph,
            &organized.materials,
        )?;
        let flat =
            crate::flatten::flatten_with_registry(&registry, "root", &organized.graph, &|id| {
                organized.materials.get(id)
            });
        assert!(flat.errors.is_empty(), "{:?}", flat.errors);
        // Interface proxies must disappear, leaving precisely the original operations.
        assert_eq!(flat.graph.nodes.len(), original.nodes.len());
        for node in &flat.graph.nodes {
            let id = flat.origins.get(&node.id).map_or(node.id, |(_, id)| *id);
            let before = original.node(id).ok_or("Unexpected operation")?;
            assert_eq!(node.kind, before.kind);
            assert_eq!(node.params, before.params);
            assert_eq!(node.text, before.text);
        }
        let canonical = |node: NodeId| flat.origins.get(&node).map_or(node, |(_, id)| *id);
        let wires: BTreeSet<_> = flat
            .graph
            .links
            .iter()
            .map(|l| {
                (
                    canonical(l.from),
                    l.output.clone(),
                    canonical(l.to),
                    l.input.clone(),
                )
            })
            .collect();
        assert_eq!(
            wires,
            original
                .links
                .iter()
                .map(|l| { (l.from, l.output.clone(), l.to, l.input.clone()) })
                .collect()
        );
        let layer = organized
            .materials
            .values()
            .find(|g| g.name == "Base Metal")
            .ok_or("Missing source layer")?;
        assert_eq!(
            layer
                .canvas
                .source
                .as_ref()
                .ok_or("Missing source metadata")?["color"],
            serde_json::json!([25, 100, 70, 255])
        );
        let again = source_groups(&organized.graph, &registry)?;
        assert!(again.materials.is_empty());
        assert_eq!(again.graph, organized.graph);
        Ok(())
    }
}

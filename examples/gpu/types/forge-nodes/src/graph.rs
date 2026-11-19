use std::collections::{BTreeMap, BTreeSet, VecDeque, btree_map};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::Registry;

/// The graph file format; files without a version are the retired v1 layout.
pub const VERSION: u32 = 2;
/// New nodes adopt definition defaults through this revision; older nodes keep theirs.
const DEFAULTS_VERSION: u32 = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NodeId(pub u32);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    /// Definition id: the kernel file's stem.
    pub kind: String,
    /// Shown instead of the definition's name when set.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
    /// Graph-space position of the node's top-left corner.
    pub pos: [f32; 2],
    /// Edited parameter values by id (colours sRGB); missing ones take the definition default.
    #[serde(default, with = "values")]
    pub params: BTreeMap<String, Vec<f32>>,
    /// Edited string parameters (paths, text, code) by id.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub text: BTreeMap<String, String>,
    /// Texture-domain resolution; `None` is the graph's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<Size>,
    /// Evaluate over the mesh's UV atlas, so an image made for these UVs (a baked or painted
    /// map) lies on them instead of wrapping onto the mesh triplanar.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub uv: bool,
    /// Definitions with a newer defaults revision use their declared legacy values.
    #[serde(default)]
    pub defaults_version: u32,
}

/// A texture-domain node's resolution as the log2 of each side.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Size {
    /// Offset from the graph's size.
    Relative([i32; 2]),
    Absolute([u32; 2]),
}

impl Size {
    /// Log2 sides for a graph of log2 side `graph`, clamped to 1 .. 8192 pixels.
    pub fn log2(self, graph: u32) -> [u32; 2] {
        let sides = match self {
            Self::Relative(d) => d.map(|d| graph as i32 + d),
            Self::Absolute(a) => a.map(|a| a as i32),
        };
        sides.map(|s| s.clamp(0, 13) as u32)
    }
}

impl Node {
    pub fn label<'a>(&'a self, registry: &'a Registry) -> &'a str {
        if !self.title.is_empty() {
            &self.title
        } else {
            registry.get(&self.kind).map_or(&self.kind, |d| &d.name)
        }
    }

    /// A parameter's packed value: the stored one, or the definition default.
    pub fn value(&self, registry: &Registry, param: &str) -> Vec<f32> {
        let stored = self.params.get(param).map(Vec::as_slice);
        registry
            .get(&self.kind)
            .and_then(|d| d.param(param).map(|p| (d, p)))
            .map_or_else(
                || stored.unwrap_or(&[]).to_vec(),
                |(d, p)| {
                    let legacy = (self.defaults_version < d.defaults_version)
                        .then_some(p.legacy_default.as_deref())
                        .flatten();
                    p.pack(stored.or(legacy))
                },
            )
    }

    /// A string parameter: the stored one, or the definition default.
    pub fn text(&self, registry: &Registry, param: &str) -> String {
        if let Some(s) = self.text.get(param) {
            return s.clone();
        }
        registry
            .get(&self.kind)
            .and_then(|d| d.param(param))
            .map_or_else(String::new, |p| p.text.clone())
    }
}

/// Output `output` of `from` feeds input `input` of `to`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Link {
    pub from: NodeId,
    pub output: String,
    pub to: NodeId,
    pub input: String,
}

/// A node parameter shown in the Paint tab's layer properties.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Exposed {
    pub node: NodeId,
    pub param: String,
    pub label: String,
}

fn default_triplanar() -> [f32; 2] {
    [1.0, 4.0]
}

fn default_size() -> u32 {
    9
}

/// A smart material: `materials/<id>.graph.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Graph {
    #[serde(default)]
    pub version: u32,
    pub name: String,
    pub nodes: Vec<Node>,
    /// Session-only deletion lineage; Undo restores it with the authored graph.
    #[serde(skip)]
    pub removed_nodes: BTreeMap<NodeId, u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interface: Option<crate::subgraph::Interface>,
    #[serde(default, skip_serializing_if = "crate::Canvas::is_empty")]
    pub canvas: crate::Canvas,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub presets: Vec<crate::ParameterPreset>,
    #[serde(default)]
    pub links: Vec<Link>,
    #[serde(default)]
    pub exposed: Vec<Exposed>,
    /// How texture-domain nodes wrap onto the mesh: repeats per unit, blend sharpness.
    #[serde(default = "default_triplanar")]
    pub triplanar: [f32; 2],
    /// Log2 side of texture-domain nodes without their own size.
    #[serde(default = "default_size")]
    pub size: u32,
}

impl Default for Graph {
    fn default() -> Self {
        Self {
            version: VERSION,
            name: String::new(),
            nodes: Vec::new(),
            removed_nodes: BTreeMap::new(),
            interface: None,
            canvas: Default::default(),
            presets: Vec::new(),
            links: Vec::new(),
            exposed: Vec::new(),
            triplanar: default_triplanar(),
            size: default_size(),
        }
    }
}

impl Graph {
    pub fn load(path: &Path) -> forge_core::Result<Self> {
        let value: serde_json::Value = forge_core::error::read_json(path)?;
        let version = value.get("version").and_then(|v| v.as_u64()).unwrap_or(0);
        if version != u64::from(VERSION) {
            return Err(forge_core::Error::format(
                path,
                format!(
                    "graph format v{} (this build reads v{VERSION})",
                    version.max(1)
                ),
            ));
        }
        serde_json::from_value(value).map_err(|source| forge_core::Error::Json {
            path: path.to_owned(),
            source,
        })
    }

    pub fn save(&self, path: &Path) -> forge_core::Result<()> {
        forge_core::error::write_json(path, self)
    }

    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }

    pub fn node_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }

    /// Adds a node of definition `kind` with default parameters.
    pub fn add(&mut self, kind: &str, pos: [f32; 2]) -> NodeId {
        let id = NodeId(self.nodes.iter().map(|n| n.id.0 + 1).max().unwrap_or(1));
        self.nodes.push(Node {
            id,
            kind: kind.to_owned(),
            title: String::new(),
            pos,
            params: BTreeMap::new(),
            text: BTreeMap::new(),
            size: None,
            uv: false,
            defaults_version: DEFAULTS_VERSION,
        });
        id
    }

    pub fn remove(&mut self, id: NodeId) {
        if self.node(id).is_some() {
            *self.removed_nodes.entry(id).or_default() += 1;
        }
        self.nodes.retain(|n| n.id != id);
        for frame in &mut self.canvas.frames {
            frame.nodes.retain(|n| *n != id);
        }
        self.links.retain(|l| l.from != id && l.to != id);
        self.exposed.retain(|e| e.node != id);
        let prefix = format!("{}/", id.0);
        for preset in &mut self.presets {
            preset.values.retain(|key, _| !key.starts_with(&prefix));
        }
        self.presets.retain(|preset| !preset.values.is_empty());
    }

    /// The node and output feeding `to`'s input `input`.
    pub fn source(&self, to: NodeId, input: &str) -> Option<(NodeId, &str)> {
        self.links
            .iter()
            .find(|l| l.to == to && l.input == input)
            .map(|l| (l.from, l.output.as_str()))
    }

    /// Links `from.output` into `to.input`, replacing what fed that input.
    pub fn connect(
        &mut self,
        registry: &Registry,
        from: NodeId,
        output: &str,
        to: NodeId,
        input: &str,
    ) -> Result<(), &'static str> {
        let (Some(a), Some(b)) = (self.node(from), self.node(to)) else {
            return Err("no such node");
        };
        let (Some(da), Some(db)) = (registry.get(&a.kind), registry.get(&b.kind)) else {
            return Err("unknown node kind");
        };
        let Some(out) = da.output(output) else {
            return Err("no such output");
        };
        let Some((_, slot)) = db.input(input) else {
            return Err("no such input");
        };
        if !slot.socket.accepts(out.socket) {
            return Err("a material only links to a material input");
        }
        if self.downstream(to).contains(&from) {
            return Err("that link would make a loop");
        }
        self.disconnect(to, input);
        self.links.push(Link {
            from,
            output: output.to_owned(),
            to,
            input: input.to_owned(),
        });
        Ok(())
    }

    pub fn disconnect(&mut self, to: NodeId, input: &str) {
        self.links.retain(|l| !(l.to == to && l.input == input));
    }

    /// `id` and every node its outputs reach.
    pub fn downstream(&self, id: NodeId) -> BTreeSet<NodeId> {
        let mut seen = BTreeSet::from([id]);
        let mut queue = VecDeque::from([id]);
        while let Some(n) = queue.pop_front() {
            for l in self.links.iter().filter(|l| l.from == n) {
                if seen.insert(l.to) {
                    queue.push_back(l.to);
                }
            }
        }
        seen
    }

    /// Nodes in evaluation order (inputs before the nodes they feed), depth first from each
    /// node nothing reads, the Output last: a node follows its inputs closely, so an
    /// evaluation holds few unread outputs at once. Nodes in or fed by a loop are left out.
    pub fn order(&self) -> Vec<NodeId> {
        let mut inputs: BTreeMap<NodeId, Vec<NodeId>> = BTreeMap::new();
        let mut read = BTreeSet::new();
        for l in &self.links {
            inputs.entry(l.to).or_default().push(l.from);
            read.insert(l.from);
        }
        let output = self.output();
        let roots = self
            .nodes
            .iter()
            .map(|n| n.id)
            .filter(|id| !read.contains(id) && Some(*id) != output)
            .chain(output);
        // false while a node's inputs are being visited, or when one is in a loop.
        let mut done: BTreeMap<NodeId, bool> = BTreeMap::new();
        let mut order = Vec::with_capacity(self.nodes.len());
        for root in roots {
            if done.contains_key(&root) {
                continue;
            }
            done.insert(root, false);
            let mut stack = vec![(root, 0)];
            while let Some(&(id, next)) = stack.last() {
                let from = inputs.get(&id).map_or(&[][..], Vec::as_slice);
                if let Some(&input) = from.get(next) {
                    if let Some(top) = stack.last_mut() {
                        top.1 += 1;
                    }
                    if let btree_map::Entry::Vacant(e) = done.entry(input) {
                        e.insert(false);
                        stack.push((input, 0));
                    }
                    continue;
                }
                stack.pop();
                let ok = from.iter().all(|i| done.get(i) == Some(&true));
                done.insert(id, ok);
                if ok {
                    order.push(id);
                }
            }
        }
        order
    }

    /// The Output node.
    pub fn output(&self) -> Option<NodeId> {
        self.nodes.iter().find(|n| n.kind == "output").map(|n| n.id)
    }

    /// Whether `param` of `node` is exposed to the Paint tab.
    pub fn exposed_index(&self, node: NodeId, param: &str) -> Option<usize> {
        self.exposed
            .iter()
            .position(|e| e.node == node && e.param == param)
    }
}

/// Saves a one-slot value as a bare number, others as arrays.
mod values {
    use std::collections::BTreeMap;

    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    #[derive(Serialize, Deserialize)]
    #[serde(untagged)]
    enum Value {
        One(f32),
        Many(Vec<f32>),
    }

    pub fn serialize<S: Serializer>(
        map: &BTreeMap<String, Vec<f32>>,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        let out: BTreeMap<&str, Value> = map
            .iter()
            .map(|(k, v)| {
                let v = match v.as_slice() {
                    [x] => Value::One(*x),
                    _ => Value::Many(v.clone()),
                };
                (k.as_str(), v)
            })
            .collect();
        out.serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        d: D,
    ) -> Result<BTreeMap<String, Vec<f32>>, D::Error> {
        let map = BTreeMap::<String, Value>::deserialize(d)?;
        Ok(map
            .into_iter()
            .map(|(k, v)| {
                let v = match v {
                    Value::One(x) => vec![x],
                    Value::Many(v) => v,
                };
                (k, v)
            })
            .collect())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    #[test]
    fn deletion_lineage_detects_reused_ids_without_changing_saved_format() {
        let mut graph = super::Graph::default();
        let id = graph.add("noise", [0.; 2]);
        let before = graph.clone();
        graph.remove(id);
        assert_eq!(graph.add("noise", [0.; 2]), id);
        assert_eq!(graph.removed_nodes.get(&id), Some(&1));
        graph.remove(id);
        assert_eq!(graph.removed_nodes.get(&id), Some(&2));
        assert!(before.removed_nodes.is_empty());
        let saved = serde_json::to_value(&graph).unwrap();
        assert!(saved.get("removed_nodes").is_none());
        let reopened: super::Graph = serde_json::from_value(saved).unwrap();
        assert!(reopened.removed_nodes.is_empty());
    }
    use super::*;

    const LEGACY_KIND: &str = "legacy_fixture";
    const LEGACY_SOURCE: &str = r#"//! name: Legacy fixture
//! category: Custom
//! description: Parameters whose defaults changed after files were saved.
//! output: output Color
//! param: outputcolor color = (0,0,0,1) "Color" legacy=(0.5,0.5,0.5,1)
//! param: channelsweights float4(0,1) = 0.3333333333333333 0.3333333333333333 0.3333333333333333 0 "Channel weights" soft legacy=(0.33,0.33,0.33,0)
//! param: inversedy enum(DirectX|OpenGL) = directx "Normal format" legacy=opengl
//! param: input2alpha bool = true "Height in alpha" legacy=false
//! domain: texture

fn eval(p: vec2<i32>) -> Out {
    return Out(p_outputcolor());
}
"#;

    fn legacy_registry() -> Registry {
        let mut files = Registry::embedded().files().clone();
        files.insert(
            format!("nodes/custom/{LEGACY_KIND}.wesl"),
            LEGACY_SOURCE.to_owned(),
        );
        Registry::from_files(files)
    }

    #[test]
    fn defaults_revision_preserves_version_one_nodes_and_pinned_definitions() {
        let registry = legacy_registry();
        let def = registry.get(LEGACY_KIND).unwrap();
        let mut files = registry.files().clone();
        files.insert(def.path.clone(), format!("//! defaults: 2\n{}", def.source));
        let updated = Registry::from_files(files);
        let mut graph = Graph::default();
        let id = graph.add(LEGACY_KIND, [0.; 2]);
        let node = graph.node_mut(id).unwrap();
        assert_eq!(node.value(&updated, "outputcolor"), [0., 0., 0., 1.]);
        node.defaults_version = 1;
        assert_eq!(node.value(&updated, "outputcolor"), [0.5, 0.5, 0.5, 1.]);
        // A saved private definition retains its own revision and defaults.
        assert_eq!(node.value(&registry, "outputcolor"), [0., 0., 0., 1.]);
        let back: Node = serde_json::from_str(&serde_json::to_string(node).unwrap()).unwrap();
        assert_eq!(back.value(&updated, "outputcolor"), [0.5, 0.5, 0.5, 1.]);
        node.params
            .insert("outputcolor".into(), vec![0.2, 0.3, 0.4, 0.5]);
        assert_eq!(node.value(&updated, "outputcolor"), [0.2, 0.3, 0.4, 0.5]);
    }

    #[test]
    fn simple_node_defaults_preserve_legacy_and_private_definitions() {
        let registry = legacy_registry();
        let mut graph = Graph::default();
        for (param, current, previous) in [
            ("outputcolor", [0., 0., 0., 1.], [0.5, 0.5, 0.5, 1.]),
            (
                "channelsweights",
                [1. / 3., 1. / 3., 1. / 3., 0.],
                [0.33, 0.33, 0.33, 0.],
            ),
        ] {
            let id = graph.add(LEGACY_KIND, [0.; 2]);
            let node = graph.node_mut(id).unwrap();
            assert_eq!(node.value(&registry, param), current);
            node.defaults_version = 0;
            assert_eq!(node.value(&registry, param), previous);
            let back: Node = serde_json::from_str(&serde_json::to_string(node).unwrap()).unwrap();
            assert_eq!(back.value(&registry, param), previous);
            let mut files = registry.files().clone();
            let def = registry.get(LEGACY_KIND).unwrap();
            // Remove only the annotation on this parameter, retaining the executable body.
            let prefix = format!("//! param: {param} ");
            let line = def
                .source
                .lines()
                .find(|line| line.starts_with(&prefix))
                .unwrap();
            let current_line = line.split(" legacy=").next().unwrap();
            files.insert(def.path.clone(), def.source.replacen(line, current_line, 1));
            let pinned = Registry::from_files(files);
            assert_eq!(node.value(&pinned, param), current);
            node.params.insert(param.into(), vec![0.2, 0.3, 0.4, 0.5]);
            assert_eq!(node.value(&registry, param), [0.2, 0.3, 0.4, 0.5]);
        }
    }

    #[test]
    fn normal_defaults_preserve_legacy_files_and_custom_pinned_definitions() {
        let registry = legacy_registry();
        let legacy: Node =
            serde_json::from_str(r#"{"id":1,"kind":"legacy_fixture","pos":[0,0]}"#).unwrap();
        assert_eq!(legacy.defaults_version, 0);
        assert_eq!(legacy.value(&registry, "inversedy"), [1.]);
        assert_eq!(legacy.value(&registry, "input2alpha"), [0.]);
        let mut graph = Graph::default();
        let id = graph.add(LEGACY_KIND, [0.; 2]);
        let current = graph.node(id).unwrap();
        assert_eq!(current.defaults_version, DEFAULTS_VERSION);
        assert_eq!(current.value(&registry, "inversedy"), [0.]);
        assert_eq!(current.value(&registry, "input2alpha"), [1.]);
        let back: Node = serde_json::from_str(&serde_json::to_string(current).unwrap()).unwrap();
        assert_eq!(back, *current);
        let mut files = registry.files().clone();
        let def = registry.get(LEGACY_KIND).unwrap();
        // A pinned custom file without migration annotations keeps its own defaults.
        files.insert(
            def.path.clone(),
            def.source
                .replace(" legacy=opengl", "")
                .replace(" legacy=false", ""),
        );
        let custom = Registry::from_files(files);
        assert_eq!(legacy.value(&custom, "inversedy"), [0.]);
        assert_eq!(legacy.value(&custom, "input2alpha"), [1.]);
    }

    #[test]
    fn connect_checks_types_and_loops() {
        let r = Registry::new();
        let mut g = Graph::default();
        let mat = g.add("material", [0.0; 2]);
        let noise = g.add("noise", [0.0; 2]);
        let first = g.add("combine", [0.0; 2]);
        let second = g.add("combine", [0.0; 2]);
        let blend = g.add("material_blend", [0.0; 2]);
        assert!(g.connect(&r, noise, "output", blend, "background").is_err());
        assert!(g.connect(&r, mat, "material", blend, "background").is_ok());
        assert!(g.connect(&r, noise, "output", first, "a").is_ok());
        assert!(g.connect(&r, first, "output", second, "a").is_ok());
        assert!(g.connect(&r, second, "output", first, "a").is_err());
        assert!(g.connect(&r, second, "output", blend, "mask").is_ok());
        let order = g.order();
        assert_eq!(order.len(), 5);
        let at = |id| order.iter().position(|n| *n == id).unwrap();
        assert!(at(noise) < at(first) && at(first) < at(second) && at(second) < at(blend));
        g.remove(first);
        assert_eq!(g.links.len(), 2);
    }

    #[test]
    fn preset_round_trips() {
        let g = crate::preset::sample_material();
        let json = serde_json::to_string(&g).unwrap();
        let back: Graph = serde_json::from_str(&json).unwrap();
        assert_eq!(back.version, VERSION);
        assert_eq!(back.nodes.len(), g.nodes.len());
        assert_eq!(back.order().len(), g.nodes.len());
        assert_eq!(back.links, g.links);
        assert!(back.output().is_some());
    }
}

//! Baked object-map usage from live, pinned shader calls; UV coverage alone is not object-aware.
use std::collections::{BTreeMap, BTreeSet};

use forge_nodes::{Graph, Registry};
use serde::{Deserialize, Serialize};
use wgpu::naga;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BakedMap {
    Position,
    WorldSpaceNormal,
    Curvature,
    AmbientOcclusion,
    Thickness,
    PartId,
    NearOtherPart,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Requirements {
    pub baked_maps: BTreeSet<BakedMap>,
    pub uv_coverage: bool,
    /// Ordinary input projection can use position/normal; this does not imply object-aware wear.
    pub projection_geometry: bool,
    /// Direct reads of packed mesh textures conservatively require every packed channel.
    pub conservative_direct_reads: bool,
}

impl Requirements {
    pub fn is_object_aware(&self) -> bool {
        !self.baked_maps.is_empty()
    }
}

/// Inspect a portable material without a GPU or another running application.
/// Uses the material's pinned implementations, expands subgraphs, and ignores disconnected nodes.
/// An error means unknown requirements; callers must not silently label it texture-only.
pub fn inspect(material: &forge_png::Material) -> Result<Requirements, String> {
    let m = &material.manifest;
    let mut registry = Registry::from_files(m.sources.clone());
    registry.set_linked(m.linked_wgsl.clone());
    let flat = crate::flatten::flatten_with_registry(&registry, "root", &m.graph, &|id| {
        m.materials.get(id)
    });
    if !flat.errors.is_empty() {
        return Err(flat.errors.join("; "));
    }
    graph(&registry, flat.graph)
}

pub(crate) fn graph(registry: &Registry, mut graph: Graph) -> Result<Requirements, String> {
    let output = graph.output().ok_or("Material has no Output node")?;
    let mut inputs = BTreeMap::<_, Vec<_>>::new();
    for link in &graph.links {
        inputs.entry(link.to).or_default().push(link.from);
    }
    let mut live = BTreeSet::new();
    let mut pending = vec![output];
    while let Some(node) = pending.pop() {
        if live.insert(node) {
            pending.extend(inputs.get(&node).into_iter().flatten().copied());
        }
    }
    graph.nodes.retain(|node| live.contains(&node.id));
    graph
        .links
        .retain(|link| live.contains(&link.to) && live.contains(&link.from));
    let (_, shaders) = registry.package_sources(&graph)?;
    let mut requirements = Requirements::default();
    for (key, source) in shaders {
        shader(&source, &mut requirements).map_err(|error| {
            format!(
                "Baked-map analysis for {}: {error}",
                key.lines().next().unwrap_or("node")
            )
        })?;
    }
    Ok(requirements)
}

fn calls(block: &naga::Block, out: &mut Vec<naga::Handle<naga::Function>>) {
    use naga::Statement;
    for statement in block {
        match statement {
            Statement::Call { function, .. } => out.push(*function),
            Statement::Block(block) => calls(block, out),
            Statement::If { accept, reject, .. } => {
                calls(accept, out);
                calls(reject, out);
            }
            Statement::Switch { cases, .. } => {
                for case in cases {
                    calls(&case.body, out);
                }
            }
            Statement::Loop {
                body, continuing, ..
            } => {
                calls(body, out);
                calls(continuing, out);
            }
            _ => {}
        }
    }
}

fn shader(source: &str, result: &mut Requirements) -> Result<(), String> {
    let module =
        naga::front::wgsl::parse_str(source).map_err(|error| error.emit_to_string(source))?;
    let mut pending = Vec::new();
    let mut seen = BTreeSet::new();
    for entry in &module.entry_points {
        reads(&module, &entry.function, result);
        calls(&entry.function.body, &mut pending);
    }
    while let Some(handle) = pending.pop() {
        if seen.insert(handle) {
            let function = &module.functions[handle];
            // The common sampler supports triplanar transport for any texture input. Its
            // position/normal reads describe projection, not the material's mask generation.
            if named(function, "triplanar") {
                result.projection_geometry = true;
                continue;
            }
            reads(&module, function, result);
            calls(&function.body, &mut pending);
        }
    }
    Ok(())
}

fn named(function: &naga::Function, helper: &str) -> bool {
    function.name.as_deref().is_some_and(|name| {
        name == helper || (name.starts_with("package_prelude") && name.ends_with(helper))
    })
}

fn reads(module: &naga::Module, function: &naga::Function, result: &mut Requirements) {
    use BakedMap::*;
    let named = |helper: &str| named(function, helper);
    for (_, expression) in function.expressions.iter() {
        let naga::Expression::GlobalVariable(global) = expression else {
            continue;
        };
        let Some(binding) = &module.global_variables[*global].binding else {
            continue;
        };
        if binding.group != 0 {
            continue;
        }
        if binding.binding == 3 && named("mesh_covered") {
            result.uv_coverage = true;
            continue;
        }
        let precise = match binding.binding {
            3 if named("mesh_position") => Some(Position),
            4 if named("mesh_normal") => Some(WorldSpaceNormal),
            4 if named("mesh_curvature") => Some(Curvature),
            5 if named("mesh_ao") => Some(AmbientOcclusion),
            5 if named("mesh_thickness") => Some(Thickness),
            5 if named("mesh_part") => Some(PartId),
            5 if named("mesh_near") => Some(NearOtherPart),
            _ => None,
        };
        if let Some(map) = precise {
            result.baked_maps.insert(map);
        } else {
            let packed: &[BakedMap] = match binding.binding {
                3 => &[Position],
                4 => &[WorldSpaceNormal, Curvature],
                5 => &[AmbientOcclusion, Thickness, PartId, NearOtherPart],
                _ => continue,
            };
            result.conservative_direct_reads = true;
            result.baked_maps.extend(packed);
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn live_calls_exclude_unused_helpers_and_keep_coverage_separate() {
        let source = "@group(0) @binding(3) var pos: texture_2d<f32>;
            @group(0) @binding(4) var nrm: texture_2d<f32>;
            @group(0) @binding(5) var extras: texture_2d<f32>;
            fn mesh_covered() -> bool { return textureLoad(pos, vec2<i32>(0), 0).w > 0.0; }
            fn mesh_curvature() -> f32 { return textureLoad(nrm, vec2<i32>(0), 0).w; }
            fn unused_thickness() -> f32 { return textureLoad(extras, vec2<i32>(0), 0).g; }
            fn nested() -> f32 { return mesh_curvature(); }
            @compute @workgroup_size(1) fn main() { if mesh_covered() { let v = nested(); } }";
        let mut result = Requirements::default();
        shader(source, &mut result).unwrap();
        assert_eq!(result.baked_maps, BTreeSet::from([BakedMap::Curvature]));
        assert!(result.uv_coverage);
        assert!(!result.conservative_direct_reads);
    }

    #[test]
    fn near_other_part_reads_as_its_own_map_and_as_part_of_a_direct_extras_read() {
        let source = "@group(0) @binding(5) var extras: texture_2d<f32>;
            fn mesh_near() -> f32 { return textureLoad(extras, vec2<i32>(0), 0).a; }
            @compute @workgroup_size(1) fn main() { let v = mesh_near(); }";
        let mut result = Requirements::default();
        shader(source, &mut result).unwrap();
        assert_eq!(result.baked_maps, BTreeSet::from([BakedMap::NearOtherPart]));
        assert!(!result.conservative_direct_reads);

        let direct = "@group(0) @binding(5) var extras: texture_2d<f32>;
            fn other() -> f32 { return textureLoad(extras, vec2<i32>(0), 0).a; }
            @compute @workgroup_size(1) fn main() { let v = other(); }";
        let mut result = Requirements::default();
        shader(direct, &mut result).unwrap();
        assert!(result.baked_maps.contains(&BakedMap::NearOtherPart));
        assert!(result.conservative_direct_reads);
    }

    #[test]
    fn disconnected_mesh_node_does_not_make_a_texture_material_object_aware() {
        let registry = Registry::embedded();
        let mut material = forge_nodes::blank_material(&registry, "Texture material");
        let curvature = material.add("surface", [0.; 2]);
        let result = graph(&registry, material.clone()).unwrap();
        assert!(!result.is_object_aware(), "{result:?}");
        let output_material = material
            .nodes
            .iter()
            .find(|node| node.kind == "material")
            .unwrap()
            .id;
        material
            .connect(&registry, curvature, "output", output_material, "roughness")
            .unwrap();
        let result = graph(&registry, material).unwrap();
        assert!(result.baked_maps.contains(&BakedMap::Curvature));
        assert!(result.baked_maps.contains(&BakedMap::NearOtherPart));
    }
}

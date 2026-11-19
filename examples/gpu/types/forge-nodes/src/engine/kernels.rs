//! Kernel compilation: linking a definition with its modules, and hot reload.

use std::collections::BTreeSet;
use std::hash::{Hash, Hasher};

use super::Engine;
use crate::codegen::generate;
use crate::graph::{Graph, Node, NodeId};
use crate::link;

#[derive(Default)]
pub(super) struct Kernel {
    pub(super) pipeline: Option<wgpu::ComputePipeline>,
    pub(super) error: Option<String>,
    /// The module files it linked (empty when linking failed).
    pub(super) modules: BTreeSet<String>,
}

impl Engine {
    /// Compiles `kind` with a node's `code` param, or the definition's default code.
    pub(super) fn compile(&self, device: &wgpu::Device, kind: &str, code: Option<&str>) -> Kernel {
        let failed = |error: String| Kernel {
            error: Some(error),
            ..Default::default()
        };
        let Some(def) = self.registry.get(kind) else {
            return failed(format!("{kind}: no such node definition"));
        };
        let default = def.code_param().map_or("", |p| p.text.as_str());
        let modules = self.registry.modules_for(kind);
        let code = code.unwrap_or(default);
        if let Some(wgsl) = self.registry.linked(kind, code) {
            let result = forge_gpu::compile(device, &def.path, wgsl).and_then(|module| {
                forge_gpu::validated(device, &def.path, || {
                    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                        label: Some(&def.path),
                        layout: Some(&self.pipeline_layout),
                        module: &module,
                        entry_point: Some("main"),
                        compilation_options: Default::default(),
                        cache: None,
                    })
                })
            });
            return match result {
                Ok(pipeline) => Kernel {
                    pipeline: Some(pipeline),
                    ..Default::default()
                },
                Err(error) => failed(error),
            };
        }
        let linked = match link::link(modules, &def.path, &def.source, code, &generate(def)) {
            Ok(linked) => linked,
            Err(e) => return failed(e),
        };
        let result = forge_gpu::compile(device, &def.path, &linked.wgsl).and_then(|module| {
            forge_gpu::validated(device, &def.path, || {
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some(&def.path),
                    layout: Some(&self.pipeline_layout),
                    module: &module,
                    entry_point: Some("main"),
                    compilation_options: Default::default(),
                    cache: None,
                })
            })
        });
        match result {
            Ok(pipeline) => Kernel {
                pipeline: Some(pipeline),
                error: None,
                modules: linked.files,
            },
            Err(e) => Kernel {
                pipeline: None,
                error: Some(linked.explain(modules, &def.path).unwrap_or(e)),
                modules: linked.files,
            },
        }
    }

    /// Definition, kernel compilation and GPU evaluation errors.
    pub fn errors(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .registry
            .errors()
            .iter()
            .map(|e| e.to_string())
            .collect();
        out.extend(self.kernels.values().filter_map(|k| k.error.clone()));
        out.extend(self.evaluation_error.clone());
        out
    }

    /// The compile error of `kind`'s kernel, once a graph used it.
    pub fn kernel_error(&self, kind: &str) -> Option<&str> {
        self.kernels.get(kind).and_then(|k| k.error.as_deref())
    }

    /// The kernel a node runs: its kind, or for nodes with a `code` param, one per code text
    /// (returned with the key).
    pub(super) fn kernel_key(&self, node: &Node) -> (String, Option<String>) {
        match self.registry.get(&node.kind).and_then(|d| d.code_param()) {
            Some(p) => {
                let code = node.text(&self.registry, &p.id);
                let mut h = std::hash::DefaultHasher::new();
                code.hash(&mut h);
                (format!("{}#{:016x}", node.kind, h.finish()), Some(code))
            }
            None => (node.kind.clone(), None),
        }
    }

    /// Validate one newly authored definition before adding it to an active graph.
    pub fn validate_kind(&self, device: &wgpu::Device, kind: &str) -> Result<(), String> {
        match self.compile(device, kind, None).error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    /// Validate all actual code-parameter variants before accepting an edited registry.
    pub fn validate_graph_kernels(
        &self,
        device: &wgpu::Device,
        graph: &Graph,
    ) -> Result<(), String> {
        let mut checked = BTreeSet::new();
        for node in &graph.nodes {
            let (key, code) = self.kernel_key(node);
            if checked.insert(key)
                && let Some(error) = self.compile(device, &node.kind, code.as_deref()).error
            {
                return Err(error);
            }
        }
        Ok(())
    }

    /// Await the device error scope instead of logging an error after a browser host has committed.
    pub async fn validate_graph_kernels_async(
        &self,
        device: &wgpu::Device,
        graph: &Graph,
    ) -> Result<(), String> {
        let (_, shaders) = self.registry.package_sources(graph)?;
        for (key, source) in shaders {
            let kind = key.split('\n').next().unwrap_or(&key);
            let label = &self
                .registry
                .get(kind)
                .ok_or("Missing candidate definition")?
                .path;
            let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
            let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(label),
                source: wgpu::ShaderSource::Wgsl(source.into()),
            });
            let _pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(label),
                layout: Some(&self.pipeline_layout),
                module: &module,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            });
            if let Some(error) = scope.pop().await {
                return Err(format!("{label}: {error}"));
            }
        }
        Ok(())
    }

    /// Compiles every definition's kernel; returns the failures.
    pub fn compile_all(&mut self, device: &wgpu::Device) -> Vec<String> {
        let kinds: Vec<String> = self.registry.defs().map(|d| d.id.clone()).collect();
        for kind in kinds {
            let kernel = self.compile(device, &kind, None);
            self.kernels.insert(kind, kernel);
        }
        self.errors()
    }

    /// Re-reads edited kernel files and recompiles what they touch; returns status lines.
    pub fn hot_reload(&mut self, device: &wgpu::Device, graph: &Graph) -> Vec<String> {
        let changes = self.registry.poll();
        if !changes.any() {
            return Vec::new();
        }
        // A module edit recompiles its importers, and kernels that failed (it may fix them).
        let touched = |k: &Kernel| {
            !changes.modules.is_empty()
                && (changes.names
                    || k.error.is_some()
                    || changes.modules.iter().any(|m| k.modules.contains(m)))
        };
        let stale: BTreeSet<String> = self
            .kernels
            .iter()
            .filter(|(key, k)| {
                let kind = key.split('#').next().unwrap_or(key);
                changes.defs.iter().any(|d| d == kind) || touched(k)
            })
            .map(|(key, _)| key.clone())
            .collect();
        self.kernels.retain(|key, _| !stale.contains(key));
        let ids: Vec<NodeId> = graph
            .nodes
            .iter()
            .filter(|n| changes.defs.contains(&n.kind) || stale.contains(&self.kernel_key(n).0))
            .map(|n| n.id)
            .collect();
        for id in ids {
            self.dirty.extend(graph.downstream(id));
        }
        self.sync(device, graph);
        let mut messages: Vec<String> = self
            .registry
            .errors()
            .iter()
            .map(|e| e.to_string())
            .collect();
        if !changes.modules.is_empty() {
            let files = changes.modules.join(", ");
            messages.push(format!("{files} reloaded ({} kernels)", stale.len()));
            messages.extend(
                self.kernels
                    .values()
                    .filter_map(|k| k.error.clone())
                    .take(1),
            );
        }
        for kind in &changes.defs {
            messages.push(match self.kernel_error(kind) {
                Some(e) => e.to_owned(),
                None => format!("{kind} reloaded"),
            });
        }
        messages
    }
}

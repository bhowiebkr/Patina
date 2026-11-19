//! Export the complete material at the asset UV-map resolution, retaining authored texture stages.
use super::*;

impl Session {
    pub fn export_target(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<(wgpu::Texture, u32), String> {
        super::subgraphs::validate_documents(self.engine.registry(), &self.graph, &self.materials)?;
        let flat = flatten::flatten_with_registry(
            self.engine.registry(),
            "document",
            &self.graph,
            &|id| self.materials.get(id),
        );
        if !flat.errors.is_empty() {
            return Err(flat.errors.join("\n"));
        }
        validate_graph(self.engine.registry(), &flat.graph)?;
        self.validate_preview_maps(&flat.graph)?;
        if !self.dirty
            && authoring::same_evaluation(&self.flat, &flat.graph)
            && let Some((texture, size)) = self.engine.output_texture(&self.flat)
        {
            return Ok((texture.clone(), size));
        }
        let mut registry = Registry::from_files(self.engine.registry().files().clone());
        registry.set_linked(self.engine.registry().linked_sources().clone());
        let mut engine = Engine::with_registry(device, registry)?;
        self.configure_engine(device, &mut engine);
        engine.set_resources(self.resources.clone());
        engine.set_source_root(&self.resource_root);
        engine.evaluate(device, queue, &flat.graph);
        let errors = engine.errors();
        if !errors.is_empty() {
            return Err(format!("PBR export: {}", errors.join("\n")));
        }
        engine
            .output_texture(&flat.graph)
            .map(|(texture, size)| (texture.clone(), size))
            .ok_or_else(|| "Connect a material to Output before exporting PBR maps".into())
    }
}

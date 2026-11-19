use material_lab::{BakeTarget, object::Object};
use patina_sdk::Ui;
use std::sync::Arc;

/// Traces object maps with the Millwright mesh baker. A changed target invalidates downstream kernels.
#[patina_sdk::node(free_layout, full_gui, color = "#356b65")]
pub struct MaterialBake {
    object: Object,
    target: BakeTarget,
    size: u32,
    revision: u32,
    status: String,
}
impl MaterialBake {
    pub fn init() -> Self {
        Self {
            object: Object::default(),
            target: BakeTarget::default(),
            size: 512,
            revision: 0,
            status: "Run to load its maps, or bake missing maps.".into(),
        }
    }
    pub fn process(
        &mut self,
        object: &Object,
        size: u32,
        rebake: u32,
    ) -> Result<BakeTarget, String> {
        let geometry = object.0.as_ref().ok_or("Connect an Object node to Bake")?;
        if self
            .object
            .0
            .as_ref()
            .is_none_or(|old| !Arc::ptr_eq(old, geometry))
            || self.size != size
            || self.revision != rebake
        {
            let start = std::time::Instant::now();
            match material_lab::bake(geometry, size, rebake != 0) {
                Ok(target) => {
                    material_lab::finish_batch()?;
                    self.status = format!(
                        "{} maps / {} / {:.2} s",
                        target.0.maps.size,
                        if target.0.cached_maps {
                            "saved bake"
                        } else {
                            "ray traced"
                        },
                        start.elapsed().as_secs_f64()
                    );
                    self.target = BakeTarget(Some(target));
                    self.object = object.clone();
                    self.size = size;
                    self.revision = rebake;
                }
                Err(e) => {
                    self.status = e.clone();
                    return Err(e);
                }
            }
        }
        Ok(self.target.clone())
    }
    pub fn ui(&mut self, ui: &mut Ui) {
        material_panel::workflow::frame(
            ui,
            "BAKE / OBJECT MAPS",
            "MILLWRIGHT SURFACE / RAY TRACED",
            290.0,
        );
        ui.pin("object", [0.0, 80.0]);
        ui.pin("size", [0.0, 118.0]);
        ui.pin("rebake", [0.0, 160.0]);
        ui.pin("out", [340.0, 263.0]);
        ui.place([16.0, 70.0]);
        ui.label("OBJECT");
        let values = [128, 256, 512, 1024, 2048];
        let mut choice = values.iter().position(|v| *v == self.size).unwrap_or(2);
        ui.place([16.0, 104.0]);
        if ui
            .dropdown(
                "Material resolution",
                &mut choice,
                &["128", "256", "512", "1024", "2048"],
            )
            .changed()
        {
            ui.set_param("size", values[choice]);
        }
        ui.place([16.0, 146.0]);
        if ui.button("Bake maps again").clicked() {
            ui.set_param("rebake", self.revision.wrapping_add(1));
        }
        ui.place([16.0, 192.0]);
        ui.label("Position / Normal / Curvature");
        ui.place([16.0, 215.0]);
        ui.label("Occlusion / Thickness / Part / Contact");
        ui.place([16.0, 253.0]);
        ui.label(&self.status);
    }
}

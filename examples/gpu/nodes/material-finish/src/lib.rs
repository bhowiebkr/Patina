use material_lab::{Material, Operator};
use material_panel::Panel;
use patina_sdk::Ui;

/// Runs Surface's finish kernel as one native Patina node.
#[patina_sdk::node(free_layout, full_gui, color = "#79533b")]
pub struct MaterialFinish {
    op: Operator,
    panel: Panel,
}
impl MaterialFinish {
    pub fn init() -> Self {
        Self {
            op: Operator::new("finish"),
            panel: Panel::default(),
        }
    }
    pub fn process(
        &mut self,
        target: &material_lab::BakeTarget,
        material: &Material,
        depth: f32,
        occlusion: f32,
        title: &str,
    ) -> Result<Material, String> {
        self.op.node.title = title.to_owned();
        self.op.source_path = Some(patina_sdk::asset!("kernel.wesl"));
        let image = self.op.run_in(
            target,
            &[("depth", depth), ("occlusion", occlusion)],
            &[material.0.as_ref()],
        )?;
        Ok(Material(Some(image)))
    }
    pub fn ui(&mut self, ui: &mut Ui) {
        self.panel.show(ui, &mut self.op);
    }
}

use material_lab::{Mask, Material, Operator};
use material_panel::Panel;
use patina_sdk::Ui;

/// Runs Surface's read kernel as one native Patina node.
#[patina_sdk::node(free_layout, full_gui, color = "#356b65")]
pub struct MaterialRead {
    op: Operator,
    panel: Panel,
}
impl MaterialRead {
    pub fn init() -> Self {
        Self {
            op: Operator::new("read"),
            panel: Panel::default(),
        }
    }
    pub fn process(
        &mut self,
        target: &material_lab::BakeTarget,
        material: &Material,
        channel: u32,
        title: &str,
    ) -> Result<Mask, String> {
        self.op.node.title = title.to_owned();
        self.op.source_path = Some(patina_sdk::asset!("kernel.wesl"));
        let image = self.op.run_in(
            target,
            &[("channel", channel as f32)],
            &[material.0.as_ref()],
        )?;
        Ok(Mask(Some(image)))
    }
    pub fn ui(&mut self, ui: &mut Ui) {
        self.panel.show(ui, &mut self.op);
    }
}

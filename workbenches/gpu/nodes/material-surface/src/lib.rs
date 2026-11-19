use material_lab::{Mask, Operator};
use material_panel::Panel;
use patina_sdk::Ui;

/// Runs Surface's surface kernel as one native Patina node.
#[patina_sdk::node(free_layout, full_gui, color = "#356b65")]
pub struct MaterialSurface {
    op: Operator,
    panel: Panel,
}
impl MaterialSurface {
    pub fn init() -> Self {
        Self {
            op: Operator::new("surface"),
            panel: Panel::default(),
        }
    }
    pub fn process(
        &mut self,
        target: &material_lab::BakeTarget,
        fact: u32,
        axis: u32,
        gain: f32,
        title: &str,
    ) -> Result<Mask, String> {
        self.op.node.title = title.to_owned();
        self.op.source_path = Some(patina_sdk::asset!("kernel.wesl"));
        let image = self.op.run_in(
            target,
            &[("fact", fact as f32), ("axis", axis as f32), ("gain", gain)],
            &[],
        )?;
        Ok(Mask(Some(image)))
    }
    pub fn ui(&mut self, ui: &mut Ui) {
        self.panel.show(ui, &mut self.op);
    }
}

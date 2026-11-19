use material_lab::{Mask, Operator};
use material_panel::Panel;
use patina_sdk::Ui;

/// Runs Surface's spread kernel as one native Patina node.
#[patina_sdk::node(free_layout, full_gui, color = "#356b65")]
pub struct MaterialSpread {
    op: Operator,
    panel: Panel,
}
impl MaterialSpread {
    pub fn init() -> Self {
        Self {
            op: Operator::new("spread"),
            panel: Panel::default(),
        }
    }
    pub fn process(
        &mut self,
        target: &material_lab::BakeTarget,
        input: &Mask,
        radius: f32,
        title: &str,
    ) -> Result<Mask, String> {
        self.op.node.title = title.to_owned();
        self.op.source_path = Some(patina_sdk::asset!("kernel.wesl"));
        let image = self
            .op
            .run_in(target, &[("radius", radius)], &[input.0.as_ref()])?;
        Ok(Mask(Some(image)))
    }
    pub fn ui(&mut self, ui: &mut Ui) {
        self.panel.show(ui, &mut self.op);
    }
}

use material_lab::{Mask, Operator};
use material_panel::Panel;
use patina_sdk::Ui;

/// Runs Surface's flow kernel as one native Patina node.
#[patina_sdk::node(free_layout, full_gui, color = "#356b65")]
pub struct MaterialFlow {
    op: Operator,
    panel: Panel,
}
impl MaterialFlow {
    pub fn init() -> Self {
        Self {
            op: Operator::new("flow"),
            panel: Panel::default(),
        }
    }
    pub fn process(
        &mut self,
        source: &Mask,
        axis: u32,
        direction: u32,
        length: f32,
        reach: u32,
        title: &str,
    ) -> Result<Mask, String> {
        self.op.node.title = title.to_owned();
        self.op.source_path = Some(patina_sdk::asset!("kernel.wesl"));
        let image = self.op.run(
            &[
                ("axis", axis as f32),
                ("direction", direction as f32),
                ("length", length),
                ("reach", reach as f32),
            ],
            &[source.0.as_ref()],
        )?;
        Ok(Mask(Some(image)))
    }
    pub fn ui(&mut self, ui: &mut Ui) {
        self.panel.show(ui, &mut self.op);
    }
}

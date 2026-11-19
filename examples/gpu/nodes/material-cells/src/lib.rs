use material_lab::{Mask, Operator};
use material_panel::Panel;
use patina_sdk::Ui;

/// Runs Surface's cells kernel as one native Patina node.
#[patina_sdk::node(free_layout, full_gui, color = "#356b65")]
pub struct MaterialCells {
    op: Operator,
    panel: Panel,
}
impl MaterialCells {
    pub fn init() -> Self {
        Self {
            op: Operator::new("cells"),
            panel: Panel::default(),
        }
    }
    pub fn process(
        &mut self,
        shape: u32,
        scale: f32,
        size: f32,
        variation: f32,
        width: f32,
        density: f32,
        stretch: f32,
        axis: u32,
        seed: u32,
        title: &str,
    ) -> Result<(Mask, Mask), String> {
        self.op.node.title = title.to_owned();
        self.op.source_path = Some(patina_sdk::asset!("kernel.wesl"));
        let image = self.op.run(
            &[
                ("shape", shape as f32),
                ("scale", scale),
                ("size", size),
                ("variation", variation),
                ("width", width),
                ("density", density),
                ("stretch", stretch),
                ("axis", axis as f32),
                ("seed", seed as f32),
            ],
            &[],
        )?;
        Ok((
            Mask(Some(image.clone())),
            Mask(Some(image.output(1, material_lab::Socket::Gray))),
        ))
    }
    pub fn ui(&mut self, ui: &mut Ui) {
        self.panel.show(ui, &mut self.op);
    }
}

use material_lab::{Mask, Operator};
use material_panel::Panel;
use patina_sdk::Ui;

/// Runs Surface's noise kernel as one native Patina node.
#[patina_sdk::node(free_layout, full_gui, color = "#356b65")]
pub struct MaterialNoise {
    op: Operator,
    panel: Panel,
}
impl MaterialNoise {
    pub fn init() -> Self {
        Self {
            op: Operator::new("noise"),
            panel: Panel::default(),
        }
    }
    #[allow(
        clippy::too_many_arguments,
        reason = "Each argument defines an editable node input"
    )]
    pub fn process(
        &mut self,
        target: &material_lab::BakeTarget,
        style: u32,
        scale: f32,
        octaves: u32,
        gain: f32,
        stretch: f32,
        axis: u32,
        warp: f32,
        seed: u32,
        title: &str,
    ) -> Result<Mask, String> {
        self.op.node.title = title.to_owned();
        self.op.source_path = Some(patina_sdk::asset!("kernel.wesl"));
        let image = self.op.run_in(
            target,
            &[
                ("style", style as f32),
                ("scale", scale),
                ("octaves", octaves as f32),
                ("gain", gain),
                ("stretch", stretch),
                ("axis", axis as f32),
                ("warp", warp),
                ("seed", seed as f32),
            ],
            &[],
        )?;
        Ok(Mask(Some(image)))
    }
    pub fn ui(&mut self, ui: &mut Ui) {
        self.panel.show(ui, &mut self.op);
    }
}

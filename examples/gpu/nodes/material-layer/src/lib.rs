use material_lab::{Mask, Material, Operator};
use material_panel::Panel;
use patina_sdk::Ui;

/// Runs Surface's layer kernel as one native Patina node.
#[patina_sdk::node(free_layout, full_gui, color = "#79533b")]
pub struct MaterialLayer {
    op: Operator,
    panel: Panel,
}
impl MaterialLayer {
    pub fn init() -> Self {
        Self {
            op: Operator::new("layer"),
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
        base: &Material,
        mask: &Mask,
        tone: &Mask,
        relief: &Mask,
        color: &str,
        rough_low: f32,
        rough_high: f32,
        metallic: f32,
        thickness: f32,
        relief_amount: f32,
        title: &str,
    ) -> Result<Material, String> {
        self.op.node.title = title.to_owned();
        self.op.source_path = Some(patina_sdk::asset!("kernel.wesl"));
        self.op.gradient(if color.is_empty() {
            "[2,0,0.1,0.25,0.2,1,1,0.3,0.6,0.45,1]"
        } else {
            color
        })?;
        let image = self.op.run_in(
            target,
            &[
                ("rough_low", rough_low),
                ("rough_high", rough_high),
                ("metallic", metallic),
                ("thickness", thickness),
                ("relief_amount", relief_amount),
            ],
            &[
                base.0.as_ref(),
                mask.0.as_ref(),
                tone.0.as_ref(),
                relief.0.as_ref(),
            ],
        )?;
        Ok(Material(Some(image)))
    }
    pub fn ui(&mut self, ui: &mut Ui) {
        self.panel.show(ui, &mut self.op);
    }
}

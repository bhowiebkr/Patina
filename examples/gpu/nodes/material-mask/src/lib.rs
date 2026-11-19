use material_lab::{Mask, Operator};
use material_panel::Panel;
use patina_sdk::Ui;

/// Runs Surface's mask kernel as one native Patina node.
#[patina_sdk::node(free_layout, full_gui, color = "#356b65")]
pub struct MaterialMask {
    op: Operator,
    panel: Panel,
}
impl MaterialMask {
    pub fn init() -> Self {
        Self {
            op: Operator::new("mask"),
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
        driver: &Mask,
        breakup: &Mask,
        amount: f32,
        from: f32,
        to: f32,
        low: f32,
        high: f32,
        title: &str,
    ) -> Result<Mask, String> {
        self.op.node.title = title.to_owned();
        self.op.source_path = Some(patina_sdk::asset!("kernel.wesl"));
        let image = self.op.run_in(
            target,
            &[
                ("amount", amount),
                ("from", from),
                ("to", to),
                ("low", low),
                ("high", high),
            ],
            &[driver.0.as_ref(), breakup.0.as_ref()],
        )?;
        Ok(Mask(Some(image)))
    }
    pub fn ui(&mut self, ui: &mut Ui) {
        self.panel.show(ui, &mut self.op);
    }
}

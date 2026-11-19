//! Common frame for the workbench's object, baking, and export controls.
use crate::{COPPER, INK, MUTED};
use patina_sdk::Ui;
pub fn frame(ui: &mut Ui, title: &str, subtitle: &str, height: f32) {
    let mut c = ui.canvas(title, [340.0, height]);
    c.rect([0.0, 0.0], [340.0, height], [38; 3], true);
    c.rect([12.0, 17.0], [3.0, 30.0], COPPER, true);
    c.text([23.0, 28.0], INK, title);
    c.text([23.0, 46.0], MUTED, subtitle);
    c.line([16.0, 57.0], [324.0, 57.0], [67; 3]);
}

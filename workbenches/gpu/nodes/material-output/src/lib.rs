use material_lab::Material;
use patina_sdk::Ui;

/// The final material. Open Material Preview to inspect its PBR channels.
#[patina_sdk::node(free_layout, color = "#79533b")]
pub struct MaterialOutput {
    ready: bool,
}
impl MaterialOutput {
    pub fn init() -> Self {
        Self { ready: false }
    }
    pub fn process(&mut self, material: &Material) -> Result<(), String> {
        self.ready = false;
        if material.0.is_none() {
            return Err("Connect a finished material".into());
        }
        material_lab::finish_batch()?;
        self.ready = true;
        Ok(())
    }
    pub fn ui(&mut self, ui: &mut Ui) {
        {
            let mut c = ui.canvas("Material output", [224.0, 140.0]);
            c.rect([0.0, 0.0], [224.0, 140.0], [38; 3], true);
            c.rect([12.0, 17.0], [3.0, 28.0], [207, 143, 98], true);
            c.text([23.0, 29.0], [225; 3], "MATERIAL OUTPUT");
            c.text([23.0, 47.0], [166; 3], "OXIDIZED COPPER / PBR");
            c.line([16.0, 60.0], [208.0, 60.0], [67; 3]);
            c.circle(
                [22.0, 82.0],
                4.0,
                if self.ready {
                    [108, 193, 180]
                } else {
                    [130; 3]
                },
                true,
            );
            c.text(
                [34.0, 86.0],
                [225; 3],
                if self.ready {
                    "MATERIAL READY"
                } else {
                    "RUN TO EVALUATE"
                },
            );
            c.text([16.0, 111.0], [166; 3], "Base / Rough / Metal / Height");
            c.text([16.0, 128.0], [166; 3], "Normal / AO / Opacity");
        }
        ui.pin("material", [0.0, 82.0]);
    }
}

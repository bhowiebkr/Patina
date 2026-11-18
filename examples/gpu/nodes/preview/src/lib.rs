use patina_gpu::{GpuImage, gpu};
use patina_sdk::{Ui, node};
use scope::{LABEL, TRACE, panel, screen};

/// The width of the node.
const WIDTH: f32 = 200.0;
/// The largest width and height of the image on the screen.
const GLASS: f32 = 164.0;
/// The top of the screen.
const TOP: f32 = 24.0;

/// Shows a small copy of the image on the screen of a monitor (10.2).
#[node(free_layout)]
pub struct Preview {
    size: [u32; 2],
    pixels: Vec<u8>,
}

impl Preview {
    pub fn init() -> Self {
        Preview { size: [0, 0], pixels: Vec::new() }
    }

    pub fn process(&mut self, image: &GpuImage) -> Result<(), String> {
        (self.size, self.pixels) = image.preview(gpu()?, 128)?;
        Ok(())
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let [w, h] = fit(self.size);
        let glass = [w + 8.0, h + 8.0];
        let x = (WIDTH - glass[0]) / 2.0;
        let size = [WIDTH, TOP + glass[1] + 14.0];
        let [iw, ih] = self.size;
        {
            let mut c = ui.canvas(&format!("Preview {iw} x {ih}"), size);
            panel(&mut c, size, "PREVIEW");
            if iw > 0 && ih > 0 {
                c.text([WIDTH - 72.0, 16.0], LABEL, &format!("{iw} x {ih}"));
            }
            screen(&mut c, [x, TOP], glass);
            if self.pixels.is_empty() {
                c.text([x + glass[0] / 2.0 - 32.0, TOP + glass[1] / 2.0 + 4.0], TRACE, "NO SIGNAL");
            }
        }
        if !self.pixels.is_empty() {
            ui.place([x + 4.0, TOP + 4.0]);
            ui.image("", self.size, &self.pixels);
        }
        ui.pin("image", [0.0, TOP + glass[1] / 2.0]);
    }
}

/// The size of the image on the screen: as wide as the glass, or as high as the glass when the
/// image is tall. The editor gives a placed image the width between equal margins (4.3).
fn fit([w, h]: [u32; 2]) -> [f32; 2] {
    if w == 0 || h == 0 {
        return [GLASS, GLASS * 9.0 / 16.0];
    }
    let ratio = h as f32 / w as f32;
    if ratio <= 1.0 { [GLASS, GLASS * ratio] } else { [GLASS / ratio, GLASS] }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_image_fills_the_screen() {
        assert_eq!(fit([64, 16]), [164.0, 41.0]);
        assert_eq!(fit([16, 64]), [41.0, 164.0]);
        assert_eq!(fit([0, 0]), [164.0, 92.25]);
        let mut preview = Preview::init();
        let mut ui = Ui::new(0, "");
        preview.ui(&mut ui);
        assert!(ui.tree().starts_with("canvas\tPreview 0 x 0\t200\t"), "{}", ui.tree());
        assert!(!ui.tree().contains("\nimage\t"), "no image before the first run");
        (preview.size, preview.pixels) = ([2, 1], vec![255; 8]);
        let mut ui = Ui::new(0, "");
        preview.ui(&mut ui);
        assert!(ui.tree().contains("\nplace\t\t18 28\nimage\t\t2\t1\t"), "{}", ui.tree());
    }
}

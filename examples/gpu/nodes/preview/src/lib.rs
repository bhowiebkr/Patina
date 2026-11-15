use patina_gpu::{GpuImage, gpu};
use patina_sdk::{Ui, node};

/// Shows a small copy of the image in the node GUI (10.2).
#[node]
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
        ui.image("Preview", self.size, &self.pixels);
    }
}

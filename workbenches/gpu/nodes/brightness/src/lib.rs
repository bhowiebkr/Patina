use patina_gpu::{GpuImage, gpu};
use patina_sdk::{Ui, node};
use scope::{LABEL, TRACE, panel, readout, screen};

/// The size of the panel, and the place and the size of the screen.
const SIZE: [f32; 2] = [188.0, 112.0];
const GLASS_AT: [f32; 2] = [14.0, 28.0];
const GLASS: [f32; 2] = [160.0, 64.0];
/// The number of columns of the histogram.
const BINS: usize = 16;

/// Gives the mean of the red channel of the image, from 0 to 1. It copies the image to the CPU.
/// The node GUI shows a histogram of the red channel and a line at the mean.
#[node(free_layout)]
pub struct Brightness {
    bins: [u32; BINS],
    mean: f64,
}

impl Brightness {
    pub fn init() -> Self {
        Brightness { bins: [0; BINS], mean: f64::NAN }
    }

    pub fn process(&mut self, image: &GpuImage) -> Result<f64, String> {
        (self.bins, self.mean) = histogram(&image.read(gpu()?)?);
        Ok(self.mean)
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        {
            let text = if self.mean.is_finite() { format!("{:.3}", self.mean) } else { "--".to_owned() };
            let mut c = ui.canvas(&format!("Brightness {text}"), SIZE);
            panel(&mut c, SIZE, "BRIGHTNESS");
            readout(&mut c, SIZE[0], &text);
            screen(&mut c, GLASS_AT, GLASS);
            let [x, y] = GLASS_AT;
            let [w, h] = GLASS;
            let (cell, most) = (w / BINS as f32, self.bins.iter().copied().max().unwrap_or(0).max(1));
            for (i, &count) in self.bins.iter().enumerate().filter(|(_, n)| **n > 0) {
                let bh = ((h - 6.0) * count as f32 / most as f32).max(1.0);
                c.rect([x + cell * i as f32 + 1.0, y + h - 3.0 - bh], [cell - 2.0, bh], TRACE, true);
            }
            if self.mean.is_finite() {
                let mx = x + w * self.mean.clamp(0.0, 1.0) as f32;
                c.line([mx, y + 2.0], [mx, y + h - 2.0], LABEL);
            }
            c.text([x, SIZE[1] - 6.0], LABEL, "0");
            c.text([x + w - 6.0, SIZE[1] - 6.0], LABEL, "1");
            c.text([x + w / 2.0 - 3.0, SIZE[1] - 6.0], LABEL, "R");
        }
        let middle = GLASS_AT[1] + GLASS[1] / 2.0;
        ui.pin("image", [0.0, middle]);
        ui.pin("out", [SIZE[0], middle]);
    }
}

/// The histogram of the red channel of RGBA pixels, and its mean from 0 to 1.
fn histogram(pixels: &[u8]) -> ([u32; BINS], f64) {
    let mut bins = [0; BINS];
    let mut sum = 0u64;
    for p in pixels.as_chunks::<4>().0 {
        sum += u64::from(p[0]);
        bins[usize::from(p[0]) * BINS / 256] += 1;
    }
    (bins, sum as f64 / (pixels.len() / 4) as f64 / 255.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_histogram_counts_the_red_channel() {
        let (bins, mean) = histogram(&[0, 9, 9, 255, 255, 0, 0, 255, 255, 0, 0, 255, 17, 0, 0, 255]);
        assert_eq!((bins[0], bins[1], bins[BINS - 1]), (1, 1, 2));
        assert!((mean - 527.0 / 4.0 / 255.0).abs() < 1e-9, "{mean}");
        let mut node = Brightness::init();
        (node.bins, node.mean) = (bins, mean);
        let mut ui = Ui::new(0, "");
        node.ui(&mut ui);
        assert!(ui.tree().starts_with("canvas\tBrightness 0.517\t188\t112\t"), "{}", ui.tree());
    }
}

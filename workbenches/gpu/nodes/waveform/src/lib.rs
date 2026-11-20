use patina_gpu::{GpuImage, gpu};
use patina_sdk::{Ui, node};
use scope::{LABEL, TRACE, panel, screen};

/// The size of the panel, and the place and the size of the screen.
const SIZE: [f32; 2] = [220.0, 132.0];
const GLASS_AT: [f32; 2] = [28.0, 26.0];
const GLASS: [f32; 2] = [176.0, 92.0];
/// The number of columns and rows of dots on the screen.
const COLS: usize = 64;
const ROWS: usize = 40;

/// Shows the brightness of the image as a waveform monitor does. The image goes from left to
/// right, and the brightness goes from black at the bottom to white at the top.
#[node(free_layout)]
pub struct Waveform {
    /// A bit for each column, in each row of brightness. Row 0 is black.
    rows: [u64; ROWS],
}

impl Waveform {
    pub fn init() -> Self {
        Waveform { rows: [0; ROWS] }
    }

    pub fn process(&mut self, image: &GpuImage) -> Result<(), String> {
        let (size, pixels) = image.preview(gpu()?, 128)?;
        self.rows = trace(size, &pixels);
        Ok(())
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        {
            let dots: u32 = self.rows.iter().map(|r| r.count_ones()).sum();
            let mut c = ui.canvas(&format!("Waveform {dots}"), SIZE);
            panel(&mut c, SIZE, "WAVEFORM");
            screen(&mut c, GLASS_AT, GLASS);
            let [x, y] = GLASS_AT;
            let [w, h] = GLASS;
            c.text([x - 15.0, y + 8.0], LABEL, "1");
            c.text([x - 15.0, y + h], LABEL, "0");
            let (cell, step) = (w / COLS as f32, (h - 6.0) / (ROWS - 1) as f32);
            for (row, bits) in self.rows.iter().enumerate() {
                let top = y + h - 4.0 - step * row as f32;
                for (start, len) in runs(*bits) {
                    c.rect([x + cell * start as f32, top], [cell * len as f32, 2.0], TRACE, true);
                }
            }
        }
        ui.pin("image", [0.0, GLASS_AT[1] + GLASS[1] / 2.0]);
    }
}

/// The rows of dots for an image of the width `w`, with RGBA pixels.
fn trace([w, _]: [u32; 2], pixels: &[u8]) -> [u64; ROWS] {
    let mut rows = [0; ROWS];
    let w = w as usize;
    if w == 0 {
        return rows;
    }
    for (i, p) in pixels.as_chunks::<4>().0.iter().enumerate() {
        let luma = 0.2126 * f32::from(p[0]) + 0.7152 * f32::from(p[1]) + 0.0722 * f32::from(p[2]);
        let row = ((luma / 255.0 * (ROWS - 1) as f32).round() as usize).min(ROWS - 1);
        rows[row] |= 1 << ((i % w) * COLS / w);
    }
    rows
}

/// The runs of set bits: the first bit and the number of bits of each run.
fn runs(mut bits: u64) -> impl Iterator<Item = (u32, u32)> {
    std::iter::from_fn(move || {
        if bits == 0 {
            return None;
        }
        let start = bits.trailing_zeros();
        let len = (bits >> start).trailing_ones();
        bits &= !(u64::MAX >> (64 - len) << start);
        Some((start, len))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_trace_puts_each_column_at_its_brightness() {
        assert_eq!(runs(0b1110_0110).collect::<Vec<_>>(), [(1, 2), (5, 3)]);
        assert_eq!(runs(u64::MAX).collect::<Vec<_>>(), [(0, 64)]);
        let rows = trace([2, 1], &[255, 255, 255, 255, 0, 0, 0, 255]);
        assert_eq!((rows[ROWS - 1], rows[0]), (1, 1 << 32), "white is at the top, black at the bottom");
        let mut waveform = Waveform::init();
        waveform.rows = rows;
        let mut ui = Ui::new(0, "");
        waveform.ui(&mut ui);
        assert!(ui.tree().starts_with("canvas\tWaveform 2\t220\t132\t"), "{}", ui.tree());
    }
}

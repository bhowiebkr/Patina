//! The pixels of the screen of the computer in one value, for one bus from the GPU to the screen.

use std::fmt::{self, Write};

use patina_sdk::{Text, Thumbnail, Ui, Viewer};

/// The width and the height of the screen, in pixels.
pub const SIZE: usize = 32;

/// The 16 colors of a pixel, as red, green and blue. Bit 3 makes a color bright, and bits 2, 1 and 0
/// are red, green and blue.
pub const PALETTE: [[u8; 3]; 16] = [
    [0, 0, 0],
    [0, 0, 170],
    [0, 170, 0],
    [0, 170, 170],
    [170, 0, 0],
    [170, 0, 170],
    [170, 85, 0],
    [170, 170, 170],
    [85, 85, 85],
    [85, 85, 255],
    [85, 255, 85],
    [85, 255, 255],
    [255, 85, 85],
    [255, 85, 255],
    [255, 255, 85],
    [255, 255, 255],
];

/// The color of each pixel, 0 to 15, row by row from the top left.
#[derive(Clone, PartialEq, Eq)]
pub struct Frame(pub [u8; SIZE * SIZE]);

impl Default for Frame {
    fn default() -> Self {
        Frame([0; SIZE * SIZE])
    }
}

impl Frame {
    /// The red, green, blue and alpha of each pixel, row by row from the top.
    fn rgba(&self, out: &mut [u8]) {
        for (pixel, &color) in out.chunks_exact_mut(4).zip(&self.0) {
            let [r, g, b] = PALETTE[usize::from(color & 15)];
            pixel.copy_from_slice(&[r, g, b, 255]);
        }
    }
}

/// Shows the number of pixels that are not black, for example "32 x 32, 12 pixels on".
impl fmt::Debug for Frame {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{SIZE} x {SIZE}, {} pixels on", self.0.iter().filter(|&&c| c != 0).count())
    }
}

/// The **Inspector** shows a frame wire as a thumbnail, as a larger screen and as the number of
/// pixels of each of the 16 colors.
impl Viewer for Frame {
    fn text(&self, out: &mut Text<'_>) -> fmt::Result {
        write!(out, "{self:?}")
    }

    fn thumbnail(&self, out: &mut Thumbnail<'_>) {
        self.rgba(out.pixels(SIZE as u32, SIZE as u32));
    }

    fn show(&self, ui: &mut Ui) {
        let mut rgba = [0; SIZE * SIZE * 4];
        self.rgba(&mut rgba);
        ui.image("Screen", [SIZE as u32; 2], &rgba);
        let mut counts = [0.0_f32; 16];
        for &color in &self.0 {
            counts[usize::from(color & 15)] += 1.0;
        }
        let top = counts.iter().copied().fold(1.0, f32::max);
        ui.plot("Pixels of each color, 0 to 15", &counts, 0.0..=top);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_text_counts_the_pixels_that_are_not_black() {
        let mut frame = Frame::default();
        frame.0[..3].copy_from_slice(&[4, 0, 15]);
        assert_eq!(format!("{frame:?}"), "32 x 32, 2 pixels on");
    }

    #[test]
    fn the_viewer_shows_the_screen_and_the_colors() {
        let mut frame = Frame::default();
        frame.0[1] = 12;
        let mut rgba = [0; SIZE * SIZE * 4];
        frame.rgba(&mut rgba);
        assert_eq!(rgba[..8], [0, 0, 0, 255, 255, 85, 85, 255]);
        let mut ui = Ui::new(1, "");
        frame.show(&mut ui);
        assert!(ui.tree().starts_with("image\tScreen\t32\t32\t"), "{}", ui.tree());
        assert!(ui.tree().contains("\nplot\tPixels of each color, 0 to 15\t"), "{}", ui.tree());
    }
}

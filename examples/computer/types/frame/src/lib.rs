//! The pixels of the screen of the computer in one value, for one bus from the GPU to the screen.

use std::fmt;

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

/// Shows the number of pixels that are not black, for example "32 x 32, 12 pixels on".
impl fmt::Debug for Frame {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{SIZE} x {SIZE}, {} pixels on", self.0.iter().filter(|&&c| c != 0).count())
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
}

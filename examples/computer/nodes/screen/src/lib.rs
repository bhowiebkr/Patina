use frame::{Frame, PALETTE, SIZE};
use patina_sdk::{Ui, node};

/// Shows the frame of the GPU, 32 x 32 pixels. When the frame changes, it writes the number of
/// pixels that are not black to the standard output.
#[node(color = "#59366f")]
pub struct Screen {
    frame: Frame,
    rgba: Vec<u8>,
}

impl Screen {
    pub fn init() -> Self {
        let mut screen = Screen { frame: Frame::default(), rgba: vec![255; SIZE * SIZE * 4] };
        screen.paint();
        screen
    }

    pub fn process(&mut self, frame: &Frame) {
        if *frame != self.frame {
            self.frame.clone_from(frame);
            self.paint();
            println!("screen {:?}", self.frame);
        }
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        ui.image("Screen", [SIZE as u32; 2], &self.rgba);
    }

    fn paint(&mut self) {
        for (pixel, &color) in self.rgba.chunks_exact_mut(4).zip(&self.frame.0) {
            pixel[..3].copy_from_slice(&PALETTE[usize::from(color % 16)]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_pixel_gets_its_palette_color() {
        let mut screen = Screen::init();
        let mut frame = Frame::default();
        frame.0[1] = 12;
        screen.process(&frame);
        assert_eq!(screen.rgba[..8], [0, 0, 0, 255, 255, 85, 85, 255]);
    }
}

use frame::{Frame, PALETTE, SIZE};
use parts::{BODY, SILK, board, glass, led, pin_names, round_rect};
use patina_sdk::{Ui, node};

/// The size of the board.
const BOARD: [f32; 2] = [196.0, 172.0];
/// The top left corner of the first pixel, and the size of a pixel.
const FIRST: [f32; 2] = [52.0, 14.0];
const PIXEL: f32 = 4.0;

/// Shows the frame of the GPU, 32 x 32 pixels. When the frame changes, it writes the frame to the
/// standard output.
#[node(color = "#59366f", free_layout)]
pub struct Screen {
    frame: Frame,
}

impl Screen {
    pub fn init() -> Self {
        Screen { frame: Frame::default() }
    }

    pub fn process(&mut self, frame: &Frame) {
        if *frame != self.frame {
            self.frame.clone_from(frame);
            println!("screen {:?}", self.frame);
        }
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        {
            let mut c = ui.canvas("Screen", BOARD);
            board(&mut c, BOARD);
            pin_names(&mut c, 78.0, &["frame"]);
            round_rect(&mut c, [42.0, 4.0], [148.0, 148.0], 6.0, BODY);
            glass(&mut c, [48.0, 10.0], [136.0, 136.0]);
            let side = SIZE as f32 * PIXEL;
            c.rect(FIRST, [side, side], PALETTE[0], true);
            // One rect for each run of one color in a row keeps the drawing small.
            for (y, row) in self.frame.0.chunks(SIZE).enumerate() {
                let mut start = 0;
                for x in 1..=SIZE {
                    if x < SIZE && row[x] % 16 == row[start] % 16 {
                        continue;
                    }
                    let color = usize::from(row[start] % 16);
                    if color != 0 {
                        let at = [FIRST[0] + start as f32 * PIXEL, FIRST[1] + y as f32 * PIXEL];
                        c.rect(at, [(x - start) as f32 * PIXEL, PIXEL], PALETTE[color], true);
                    }
                    start = x;
                }
            }
            c.text([48.0, 166.0], SILK, "VIDEO 32 x 32");
            c.text([148.0, 166.0], SILK, "PWR");
            led(&mut c, [180.0, 162.0], 3.5, true);
        }
        ui.pin("frame", [0.0, 78.0]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_run_of_pixels_gets_its_palette_color() {
        let mut screen = Screen::init();
        let mut frame = Frame::default();
        frame.0[1] = 12;
        frame.0[2] = 12;
        screen.process(&frame);
        let mut ui = Ui::new(0, "");
        screen.ui(&mut ui);
        assert!(ui.tree().contains("\trect 56 14 8 4 #ff5555 1"), "{}", ui.tree());
    }
}

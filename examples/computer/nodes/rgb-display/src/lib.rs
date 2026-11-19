use parts::{SILK, TEXT, edges, package, window};
use patina_sdk::{Ui, node};

/// The width of the drawing of the chip and the size of a pixel.
const WIDTH: f32 = 220.0;
const PIXEL: f32 = 8.0;

/// A display of 16 x 16 pixels with 4 bits for each of red, green and blue, as the RGB display of
/// Digital Logic Sim 2. At a rising edge of **clock**, **reset** clears the back buffer, or
/// **write** sets the pixel at **address**. Then **refresh** copies the back buffer to the screen.
/// The address is the row x 16 plus the column, and row 0 is at the bottom. The outputs are the
/// red, green and blue of the pixel at **address** on the screen. When the screen changes, it
/// writes each row as 3 hex digits for each pixel, from the top row.
#[node(color = "#59366f", free_layout)]
pub struct RgbDisplay {
    screen: [u16; 256],
    back: [u16; 256],
    clock: bool,
}

impl RgbDisplay {
    pub fn init() -> Self {
        RgbDisplay { screen: [0; 256], back: [0; 256], clock: false }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn process(&mut self, address: &u8, red: &u8, green: &u8, blue: &u8, reset: &bool, write: &bool, refresh: &bool, clock: &bool) -> (u8, u8, u8) {
        if *clock && !self.clock {
            if *reset {
                self.back = [0; 256];
            } else if *write {
                self.back[usize::from(*address)] = u16::from(*red & 15) | u16::from(*green & 15) << 4 | u16::from(*blue & 15) << 8;
            }
            if *refresh && self.screen != self.back {
                self.screen = self.back;
                println!("rgb {}", self.rows());
            }
        }
        self.clock = *clock;
        let [r, g, b] = channels(self.screen[usize::from(*address)]);
        (r, g, b)
    }

    fn rows(&self) -> String {
        let row = |y: usize| self.screen[y * 16..][..16].iter().map(|&p| channels(p).map(|c| format!("{c:x}")).concat()).collect::<String>();
        (0..16).rev().map(row).collect::<Vec<_>>().join(" ")
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let e = edges(&[8, 8, 8, 8, 0, 0, 0, 0], &[8, 8, 8]);
        let lit = self.screen.iter().filter(|&&p| p != 0).count();
        let mut c = ui.canvas(&format!("RGB display, pixels on: {lit}"), [WIDTH, e.height]);
        let [x0, _] = package(&mut c, &e, WIDTH);
        c.text([x0 + 18.0, 24.0], TEXT, "RGB");
        c.text([x0 + 52.0, 24.0], SILK, "16 x 16 x 12");
        let [left, top] = [x0 + 18.0, 38.0];
        window(&mut c, [left - 4.0, top - 4.0], [16.0 * PIXEL + 8.0; 2]);
        // One rect for each run of one color in a row keeps the drawing small.
        for (y, row) in self.screen.chunks(16).enumerate() {
            let mut start = 0;
            for x in 1..=16 {
                if x < 16 && row[x] == row[start] {
                    continue;
                }
                if row[start] != 0 {
                    let at = [left + start as f32 * PIXEL, top + (15 - y) as f32 * PIXEL];
                    c.rect(at, [(x - start) as f32 * PIXEL, PIXEL], channels(row[start]).map(|v| v * 17), true);
                }
                start = x;
            }
        }
    }
}

/// The red, green and blue of a pixel, 0 to 15 each.
fn channels(pixel: u16) -> [u8; 3] {
    [0, 4, 8].map(|shift| (pixel >> shift & 15) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pixel_shows_its_color_after_a_refresh_at_a_rising_edge() {
        let mut rgb = RgbDisplay::init();
        // Address 1 is row 0, column 1: the bottom row. The high bits of each channel do not count.
        rgb.process(&1, &0x1f, &0, &5, &false, &true, &true, &true);
        assert_eq!(rgb.process(&1, &0, &0, &0, &false, &false, &false, &false), (15, 0, 5));
        assert!(rgb.rows().ends_with(" 000f05000000000000000000000000000000000000000000"), "{}", rgb.rows());
        let mut ui = Ui::new(0, "");
        rgb.ui(&mut ui);
        assert!(ui.tree().contains("\trect 54 158 8 8 #ff0055 1"), "{}", ui.tree());
        rgb.process(&1, &0, &0, &0, &true, &false, &true, &true);
        assert_eq!(rgb.process(&1, &0, &0, &0, &false, &false, &false, &false), (0, 0, 0));
    }
}

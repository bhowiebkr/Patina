use frame::{Frame, SIZE};
use patina_sdk::{Ui, node};

/// A small video chip on the IO ports of the CPU. It keeps a frame of 32 x 32 pixels with 16 colors
/// and gives it to the screen. When `wr` is on, the CPU writes `data` to the register at `port`:
///
/// - `address`: X, the column of the next pixel, 0 to 31.
/// - `address + 1`: Y, the row of the next pixel, 0 to 31.
/// - `address + 2`: plot. The pixel at X and Y gets the color `data`, 0 to 15. Then X moves one
///   pixel to the right. After column 31, it goes to column 0 of the next row.
/// - `address + 3`: clear. All pixels get the color `data`, and X and Y go to 0.
///
/// LD at `address + 2` gives the color of the pixel at X and Y, and X does not move. A game can
/// read the pixel in front of a player before it moves. At the other ports, the byte on `bus`
/// goes through.
#[node(color = "#59366f")]
pub struct Gpu {
    frame: Frame,
    x: usize,
    y: usize,
}

impl Gpu {
    pub fn init() -> Self {
        Gpu { frame: Frame::default(), x: 0, y: 0 }
    }

    pub fn process(&mut self, bus: &u8, data: &u8, port: &u8, wr: &bool, address: u8) -> (Frame, u8) {
        let read = if port.wrapping_sub(address) == 2 { self.frame.0[self.y * SIZE + self.x] } else { *bus };
        if *wr {
            let value = usize::from(*data);
            match port.wrapping_sub(address) {
                0 => self.x = value % SIZE,
                1 => self.y = value % SIZE,
                2 => {
                    self.frame.0[self.y * SIZE + self.x] = data % 16;
                    self.x = (self.x + 1) % SIZE;
                    if self.x == 0 {
                        self.y = (self.y + 1) % SIZE;
                    }
                }
                3 => {
                    self.frame.0.fill(data % 16);
                    (self.x, self.y) = (0, 0);
                }
                _ => {}
            }
        }
        (self.frame.clone(), read)
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        ui.label(&format!("X {}  Y {}", self.x, self.y));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plot_moves_to_the_next_row_after_the_last_column_and_clear_fills() {
        let mut gpu = Gpu::init();
        let write = |gpu: &mut Gpu, port: u8, data: u8| gpu.process(&0, &data, &port, &true, 9).0;
        write(&mut gpu, 9, 31);
        write(&mut gpu, 10, 2);
        write(&mut gpu, 11, 4);
        let frame = write(&mut gpu, 11, 18);
        assert_eq!((frame.0[2 * SIZE + 31], frame.0[3 * SIZE]), (4, 2));
        assert_eq!((gpu.x, gpu.y), (1, 3));
        // A write to another port or with wr off changes nothing.
        gpu.process(&0, &7, &8, &true, 9);
        gpu.process(&0, &7, &11, &false, 9);
        assert_eq!(gpu.frame.0.iter().filter(|&&c| c != 0).count(), 2);
        // A read of the plot port gives the pixel at X and Y. Other ports give the bus byte.
        write(&mut gpu, 9, 31);
        write(&mut gpu, 10, 2);
        assert_eq!(gpu.process(&5, &0, &11, &false, 9).1, 4);
        assert_eq!(gpu.process(&5, &0, &10, &false, 9).1, 5);
        assert_eq!(gpu.x, 31);
        let frame = write(&mut gpu, 12, 1);
        assert!(frame.0.iter().all(|&c| c == 1));
        assert_eq!((gpu.x, gpu.y), (0, 0));
    }
}

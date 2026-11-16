use patina_sdk::{Ui, node};

const N: usize = 96;
const STEP: f32 = 3.0;
const LEFT: f32 = 52.0;
const ROW: f32 = 20.0;
const HIGH: [u8; 3] = [223, 166, 75];
const FILL: [u8; 3] = [74, 58, 34];
const LOW: [u8; 3] = [115, 115, 110];
const GRID: [u8; 3] = [48, 48, 48];
const TEXT: [u8; 3] = [178, 178, 173];
const DEFAULT: [&str; 8] = ["d0", "d1", "d2", "d3", "d4", "d5", "d6", "d7"];

/// A logic analyzer: draws the last 96 frames of 8 signals as waveforms, the newest at the
/// right. `names` labels the channels, separated by commas.
#[node]
pub struct Analyzer {
    samples: [u8; N],
    next: usize,
    names: String,
}

impl Analyzer {
    pub fn init() -> Self {
        Analyzer { samples: [0; N], next: 0, names: String::new() }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn process(&mut self, d0: &bool, d1: &bool, d2: &bool, d3: &bool, d4: &bool, d5: &bool, d6: &bool, d7: &bool, names: &str) {
        let bits = [d0, d1, d2, d3, d4, d5, d6, d7];
        self.samples[self.next] = bits.iter().enumerate().fold(0, |s, (i, &&b)| s | u8::from(b) << i);
        self.next = (self.next + 1) % N;
        if self.names != names {
            names.clone_into(&mut self.names);
        }
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let now = self.samples[(self.next + N - 1) % N];
        let text: String = (0..8).map(|ch| if now >> ch & 1 == 1 { '1' } else { '0' }).collect();
        let mut canvas = ui.canvas(&format!("d0 to d7: {text}"), [LEFT + STEP * N as f32 + 4.0, ROW * 8.0]);
        for k in (0..=N).step_by(16) {
            let x = LEFT + STEP * k as f32;
            canvas.line([x, 0.0], [x, ROW * 8.0], GRID);
        }
        let mut names = self.names.split(',').map(str::trim);
        for (ch, default) in DEFAULT.iter().enumerate() {
            let (hi, lo) = (ROW * ch as f32 + 4.0, ROW * ch as f32 + 16.0);
            canvas.text([4.0, lo - 1.0], TEXT, names.next().filter(|n| !n.is_empty()).unwrap_or(default));
            // The oldest sample is at `next`. Each run of one level draws as one segment.
            let bit = |k: usize| self.samples[(self.next + k) % N] >> ch & 1 == 1;
            let mut start = 0;
            for k in 1..=N {
                if k < N && bit(k) == bit(start) {
                    continue;
                }
                let (x0, x1) = (LEFT + STEP * start as f32, LEFT + STEP * k as f32);
                if bit(start) {
                    canvas.rect([x0, hi], [x1 - x0, lo - hi], FILL, true);
                    canvas.line([x0, hi], [x1, hi], HIGH);
                } else {
                    canvas.line([x0, lo], [x1, lo], LOW);
                }
                if k < N {
                    canvas.line([x1, hi], [x1, lo], HIGH);
                }
                start = k;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_last_samples_and_the_names() {
        let mut analyzer = Analyzer::init();
        for k in 0..N + 2 {
            let on = k % 2 == 1;
            analyzer.process(&on, &false, &true, &false, &false, &false, &false, &on, "clk, out");
        }
        assert_eq!(analyzer.next, 2);
        assert_eq!(analyzer.samples[0], 0b0000_0100);
        assert_eq!(analyzer.samples[1], 0b1000_0101);
        assert_eq!(analyzer.names, "clk, out");
    }
}

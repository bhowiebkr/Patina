use super::*;
use patina_sdk::Canvas;

pub fn axes(c: &mut Canvas, [x, y, w, h]: [f32; 4], hz: f32, columns: usize, floor: f32) {
    for i in 0..=4 {
        let at = i as f32 / 4.0;
        c.text(
            [12.0, y + h * (1.0 - at) + 4.0],
            MUTED,
            format!("{:.0}", hz * at),
        );
        let seconds = History::seconds(columns) * (1.0 - at);
        c.text(
            [x + w * at - 10.0, y + h + 20.0],
            MUTED,
            &if i == 4 {
                "0.0".to_owned()
            } else {
                format!("-{seconds:.1}")
            },
        );
    }
    c.text([12.0, y - 8.0], MUTED, "Hz");
    c.text([x + w / 2.0 - 34.0, y + h + 38.0], MUTED, "seconds ago");
    let blocks = (w / 4.0) as usize;
    for i in 0..blocks {
        c.rect(
            [x + i as f32 * 4.0, y + h + 52.0],
            [4.0, 8.0],
            color(i as f32 / (blocks - 1) as f32),
            true,
        );
    }
    c.text([x, y + h + 74.0], MUTED, format!("{floor:.0} dBFS"));
    c.text([x + w - 45.0, y + h + 74.0], MUTED, "0 dBFS");
}

pub fn pixels(history: &History, pixels: &mut [u8], hz: f32, span: usize, floor: f32) {
    for y in 0..128 {
        let low = ((127 - y) as f32 / 128.0 * hz * WINDOW as f32 / RATE).floor() as usize;
        let high = ((128 - y) as f32 / 128.0 * hz * WINDOW as f32 / RATE).ceil() as usize;
        for x in 0..256 {
            let mut level = -100.0_f32;
            for column in x * span / 256..(x + 1) * span / 256 {
                for bin in low..=high.min(WINDOW / 2) {
                    level = level.max(history.level(column, bin, span));
                }
            }
            let rgb = color(((level - floor) / -floor).clamp(0.0, 1.0));
            pixels[(y * 256 + x) * 4..(y * 256 + x + 1) * 4]
                .copy_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
        }
    }
}

fn color(value: f32) -> [u8; 3] {
    const STOPS: [[u8; 3]; 6] = [
        [21, 17, 29],
        [68, 25, 104],
        [59, 82, 139],
        [33, 145, 140],
        [94, 201, 98],
        [253, 231, 37],
    ];
    let at = value.clamp(0.0, 1.0) * 5.0;
    let i = (at as usize).min(4);
    std::array::from_fn(|c| {
        (f32::from(STOPS[i][c]) * (1.0 - (at - i as f32))
            + f32::from(STOPS[i + 1][c]) * (at - i as f32)) as u8
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn level_colors_increase_in_brightness() {
        let mut previous = 0.0;
        for i in 0..=100 {
            let [r, g, b] = color(i as f32 / 100.0);
            let luminance = 0.2126 * f32::from(r) + 0.7152 * f32::from(g) + 0.0722 * f32::from(b);
            assert!(luminance >= previous - 1.0);
            previous = luminance;
        }
    }
}

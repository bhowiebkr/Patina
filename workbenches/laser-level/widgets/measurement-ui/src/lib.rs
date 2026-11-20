use patina_sdk::{Canvas, Mode, Ui};
pub mod beam;
pub mod results;
pub mod surface;
pub mod surface_camera;
pub mod surface_render;
pub mod survey;
pub const INK: [u8; 3] = [238, 238, 235];
pub const MUTED: [u8; 3] = [178, 178, 173];
pub const EDGE: [u8; 3] = [69, 69, 69];
pub const AMBER: [u8; 3] = [227, 184, 105];
pub const TEAL: [u8; 3] = [121, 212, 200];
pub const PINK: [u8; 3] = [222, 143, 186];
pub const VIOLET: [u8; 3] = [185, 155, 223];
pub const RED: [u8; 3] = [237, 123, 120];

pub fn rounded(c: &mut Canvas, [x, y, w, h]: [f32; 4], color: [u8; 3]) {
    c.path(
        &format!(
            "M {} {y} H {} Q {} {y} {} {} V {} Q {} {} {} {} H {} Q {x} {} {x} {} V {} Q {x} {y} {} {y} Z",
            x + 6.0,
            x + w - 6.0,
            x + w,
            x + w,
            y + 6.0,
            y + h - 6.0,
            x + w,
            y + h,
            x + w - 6.0,
            y + h,
            x + 6.0,
            y + h,
            y + h - 6.0,
            y + 6.0,
            x + 6.0
        ),
        color,
        true,
    );
}
pub fn panel(ui: &mut Ui, size: [f32; 2], title: &str, subtitle: &str, color: [u8; 3]) {
    if ui.mode() == Mode::Large {
        ui.label(title);
        ui.label(subtitle);
        return;
    }
    let mut c = ui.canvas(title, size);
    rounded(&mut c, [0.0, 0.0, size[0], size[1]], EDGE);
    rounded(&mut c, [1.0, 1.0, size[0] - 2.0, size[1] - 2.0], [38, 38, 38]);
    c.rect([14.0, 15.0], [3.0, 27.0], color, true);
    c.text([26.0, 28.0], INK, title);
    c.text([26.0, 48.0], MUTED, subtitle);
    c.line([16.0, 60.0], [size[0] - 16.0, 60.0], EDGE);
}
pub fn place(ui: &mut Ui, at: [f32; 2]) {
    if ui.mode() != Mode::Large {
        ui.place(at);
    }
}
pub fn field<T: patina_sdk::Number + std::fmt::Display>(
    ui: &mut Ui,
    at: [f32; 2],
    label: &str,
    key: &str,
    value: &mut T,
    step: T,
    unit: &str,
) {
    place(ui, at);
    if ui.number_unit(label, value, step, unit).changed() {
        ui.set_param(key, *value);
    }
}
pub fn plot_frame(c: &mut Canvas, [x, y, w, h]: [f32; 4]) {
    c.rect([x, y], [w, h], [18, 18, 18], true);
    for i in 0..=4 {
        let t = i as f32 / 4.0;
        c.line([x + t * w, y], [x + t * w, y + h], [48, 48, 48]);
        c.line([x, y + t * h], [x + w, y + t * h], [48, 48, 48]);
    }
}
pub fn digits(c: &mut Canvas, at: [f32; 2], value: &str, color: [u8; 3]) {
    let mut x = at[0];
    let y = at[1];
    for ch in value.chars().take(12) {
        if ch == '.' {
            c.rect([x, y + 33.0], [4.0, 4.0], color, true);
            x += 9.0;
            continue;
        }
        if ch == '-' {
            c.rect([x + 3.0, y + 17.0], [17.0, 3.0], color, true);
            x += 27.0;
            continue;
        }
        let bits = match ch {
            '0' => 0x3f,
            '1' => 6,
            '2' => 0x5b,
            '3' => 0x4f,
            '4' => 0x66,
            '5' => 0x6d,
            '6' => 0x7d,
            '7' => 7,
            '8' => 0x7f,
            '9' => 0x6f,
            _ => 0,
        };
        for (i, r) in [
            [3.0, 0.0, 17.0, 3.0],
            [20.0, 3.0, 3.0, 14.0],
            [20.0, 20.0, 3.0, 14.0],
            [3.0, 34.0, 17.0, 3.0],
            [0.0, 20.0, 3.0, 14.0],
            [0.0, 3.0, 3.0, 14.0],
            [3.0, 17.0, 17.0, 3.0],
        ]
        .iter()
        .enumerate()
        {
            c.rect([x + r[0], y + r[1]], [r[2], r[3]], if bits & (1 << i) != 0 { color } else { [48, 48, 46] }, true);
        }
        x += 29.0;
    }
}

pub fn station_map(
    ui: &mut Ui,
    run: &measurement::core::survey::Run,
    selected: usize,
    size: [f32; 2],
) -> Option<usize> {
    let mut c = ui.canvas("Station map / select a station", size);
    let (origin, area, scale) = station_area(run, size);
    plot_frame(&mut c, area);
    let mut picked = None;
    let mut nearest = 12.0;
    let mut visited = [0_u64; 64];
    for visit in &run.visits {
        if let Some(word) = visited.get_mut(visit.station / 64) {
            *word |= 1 << (visit.station % 64);
        }
    }
    for (i, s) in run.stations.iter().enumerate() {
        let at = [
            area[0] + ((s.x_mm - origin[0]) * scale) as f32,
            area[1] + area[3] - ((s.y_mm - origin[1]) * scale) as f32,
        ];
        for p in c.pointer() {
            if let patina_sdk::Pointer::Down(pt) = p
                && (pt[0] - at[0]).hypot(pt[1] - at[1]) < nearest
            {
                nearest = (pt[0] - at[0]).hypot(pt[1] - at[1]);
                picked = Some(i);
            }
        }
        let done = visited.get(i / 64).is_some_and(|word| word & (1 << (i % 64)) != 0);
        let radius = if run.stations.len() > 100 { 1.5 } else { 4.0 };
        c.circle(at, radius, if done { TEAL } else { MUTED }, true);
        if i == selected {
            c.circle(at, 8.0, AMBER, false);
        }
        if i == run.zero {
            c.circle(at, 11.0, VIOLET, false);
        }
        if run.stations.len() <= 36 {
            c.text(
                [(at[0] + 8.0).min(size[0] - 40.0), at[1] - 5.0],
                if i == selected { AMBER } else { MUTED },
                format_args!("{}{}", i + 1, if i == run.zero { " Z" } else { "" }),
            );
        }
    }
    c.text([24.0, size[1] - 6.0], MUTED, "X / Y in mm     Z = zero reference");
    picked
}

fn station_area(run: &measurement::core::survey::Run, size: [f32; 2]) -> ([f64; 2], [f32; 4], f64) {
    let mut lo = [f64::INFINITY; 2];
    let mut hi = [f64::NEG_INFINITY; 2];
    for p in &run.stations {
        for (i, value) in [p.x_mm, p.y_mm].into_iter().enumerate() {
            lo[i] = lo[i].min(value);
            hi[i] = hi[i].max(value);
        }
    }
    if run.stations.is_empty() {
        lo = [0.; 2];
        hi = [1.; 2];
    }
    let span = [hi[0] - lo[0], hi[1] - lo[1]];
    let available = size.map(|v| (v - 48.).max(1.));
    let scale = (available[0] as f64 / span[0].max(1e-9)).min(available[1] as f64 / span[1].max(1e-9));
    let [w, h] = span.map(|v| (v * scale) as f32);
    (lo, [24. + (available[0] - w) / 2., 20. + (available[1] - h) / 2., w, h], scale)
}

#[cfg(test)]
mod tests {
    #[test]
    fn dense_station_map_picks_the_nearest_point() {
        let run = measurement::core::survey::Run::grid("Coin", 41, 41, 0.75, 0.75);
        let mut ui = patina_sdk::Ui::new(0, "0 down 93 162");
        assert_eq!(super::station_map(&mut ui, &run, 0, [328., 190.]), Some(0));
    }
    #[test]
    fn maps_preserve_square_rectangular_and_line_extents() {
        use measurement::core::survey::Run;
        let (_, square, _) = super::station_area(&Run::grid("Coin", 41, 41, 0.75, 0.75), [440., 440.]);
        assert_eq!(square, [24., 20., 392., 392.]);
        let (_, rectangle, _) = super::station_area(&Run::grid("Plate", 3, 3, 2., 1.), [440., 440.]);
        assert_eq!(rectangle[2], rectangle[3] * 2.);
        let (_, line, scale) = super::station_area(&Run::line("Line", 5, 1.), [440., 440.]);
        assert_eq!(line[3], 0.);
        assert!(scale.is_finite());
    }
}

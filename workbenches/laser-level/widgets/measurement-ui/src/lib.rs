use patina_sdk::{Canvas, Mode, Ui};
pub mod beam;
pub mod results;
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
    let maxx = run.stations.iter().map(|s| s.x_mm).fold(1.0, f64::max);
    let maxy = run.stations.iter().map(|s| s.y_mm).fold(1.0, f64::max);
    plot_frame(&mut c, [24.0, 20.0, size[0] - 48.0, size[1] - 48.0]);
    let mut picked = None;
    for (i, s) in run.stations.iter().enumerate() {
        let at = [
            24.0 + (size[0] - 48.0) * (s.x_mm / maxx) as f32,
            size[1] - 28.0 - (size[1] - 48.0) * (s.y_mm / maxy) as f32,
        ];
        for p in c.pointer() {
            if let patina_sdk::Pointer::Down(pt) = p
                && (pt[0] - at[0]).hypot(pt[1] - at[1]) < 12.0
            {
                picked = Some(i);
            }
        }
        let visited = run.visits.iter().any(|v| v.station == i);
        c.circle(at, 4.0, if visited { TEAL } else { MUTED }, true);
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

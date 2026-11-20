use crate::{AMBER, INK, MUTED, TEAL, place, plot_frame};
use measurement::{Beam, core::frame::Roi};
use patina_sdk::{Mode, Pointer, Ui};

/// Image and profile use the same oriented sensor coordinates.
#[derive(Default)]
pub struct Preview {
    pixels: Vec<u8>,
    drag: Option<[f32; 2]>,
}
impl Preview {
    pub fn ui(&mut self, ui: &mut Ui, beam: &Beam) -> Option<Roi> {
        let Some(im) = &beam.frame.image else {
            ui.label("Waiting for a camera frame");
            return None;
        };
        let orientation = beam.frame.orientation;
        self.pixels.resize(256 * 176 * 4, 255);
        let (vw, vh) = orientation.view_size(im.width as f64, im.height as f64);
        for y in 0..176 {
            for x in 0..256 {
                let level = if x < 168 && y < 168 {
                    let (sx, sy) = orientation.to_image(
                        (x as f64 + 0.5) * vw / 168.0,
                        (y as f64 + 0.5) * vh / 168.0,
                        im.width as f64,
                        im.height as f64,
                    );
                    im.data[(sy as usize).min(im.height - 1) * im.width + (sx as usize).min(im.width - 1)]
                } else {
                    18
                };
                self.pixels[(y * 256 + x) * 4..(y * 256 + x) * 4 + 3].fill(level);
            }
        }
        if ui.mode() == Mode::Large {
            ui.image("Sensor / oriented view", [256, 176], &self.pixels);
            let values: Vec<f32> = beam.detail.profile.iter().map(|v| *v as f32).collect();
            ui.plot_axes("Intensity profile", &values, 0.0..=255.0, 0.0..=values.len() as f32, ["px", "level"]);
            return None;
        }
        place(ui, [16.0, 134.0]);
        ui.image("Sensor / oriented view", [256, 176], &self.pixels);
        place(ui, [16.0, 134.0]);
        let mut c = ui.canvas("Sensor image with linked profile", [440.0, 304.0]);
        plot_frame(&mut c, [302.0, 0.0, 134.0, 288.0]);
        let at = |sx: f64, sy: f64| {
            let (x, y) = orientation.to_view(sx, sy, im.width as f64, im.height as f64);
            [288.0 * (x / vw) as f32, 288.0 * (y / vh) as f32]
        };
        if let Ok(fix) = &beam.fix {
            let (a, b) = match orientation.axis() {
                measurement::core::frame::LineAxis::Vertical => {
                    (at(fix.position_px, 0.0), at(fix.position_px, im.height as f64))
                }
                measurement::core::frame::LineAxis::Horizontal => {
                    (at(0.0, fix.position_px), at(im.width as f64, fix.position_px))
                }
            };
            c.line(a, b, TEAL);
            let yy = a[1];
            c.line([302.0, yy], [436.0, yy], TEAL);
            c.text([305.0, 16.0], INK, format_args!("{:.1} px FWHM", fix.fwhm_px));
        }
        let profile = &beam.detail.profile;
        let mut last = None;
        for (i, &value) in profile.iter().enumerate().step_by((profile.len() / 256).max(1)) {
            let px = (i + beam.detail.offset_px) as f64;
            let p = match orientation.axis() {
                measurement::core::frame::LineAxis::Vertical => at(px, 0.0),
                _ => at(0.0, px),
            };
            let point = [304.0 + value as f32 / 255.0 * 130.0, p[1]];
            if let Some(previous) = last {
                c.line(previous, point, AMBER);
            }
            last = Some(point);
        }
        if let Some(r) = beam.config.roi {
            let a = at(r.x as f64, r.y as f64);
            let b = at((r.x + r.width) as f64, (r.y + r.height) as f64);
            c.rect([a[0].min(b[0]), a[1].min(b[1])], [(a[0] - b[0]).abs(), (a[1] - b[1]).abs()], TEAL, false);
        }
        c.text([0.0, 302.0], MUTED, "DRAG IMAGE TO SET ROI");
        c.text([302.0, 302.0], MUTED, "0       LEVEL      255");
        c.grab_rect([0.0, 0.0], [288.0, 288.0]);
        let mut roi = None;
        for event in c.pointer() {
            match *event {
                Pointer::Down(p) if (0.0..288.0).contains(&p[0]) && (0.0..288.0).contains(&p[1]) => self.drag = Some(p),
                Pointer::Up(p) => {
                    if let Some(a) = self.drag.take() {
                        let map = |p: [f32; 2]| {
                            orientation.to_image(
                                p[0].clamp(0.0, 288.0) as f64 / 288.0 * vw,
                                p[1].clamp(0.0, 288.0) as f64 / 288.0 * vh,
                                im.width as f64,
                                im.height as f64,
                            )
                        };
                        let (a, b) = (map(a), map(p));
                        let r = Roi {
                            x: a.0.min(b.0).floor() as usize,
                            y: a.1.min(b.1).floor() as usize,
                            width: (a.0 - b.0).abs().ceil() as usize,
                            height: (a.1 - b.1).abs().ceil() as usize,
                        };
                        if r.width >= 3 && r.height >= 3 {
                            roi = r.clamped(im.width, im.height);
                        }
                    }
                }
                _ => {}
            }
        }
        roi
    }
}

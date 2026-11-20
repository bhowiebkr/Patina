//! The node preview and the dock share geometry, colors and camera controls.
use crate::{INK, MUTED, surface_camera::Camera};
use measurement::{
    ResultData,
    surface::{Mesh, Surface, color},
};
use patina_sdk::{Key, Pointer, Ui};
use std::{fmt::Write, sync::Arc};

pub struct Plot {
    pub mesh: Mesh,
    pub camera: Camera,
    pub scale: f32,
    pub channel: usize,
    pub source: Option<Arc<Surface>>,
    built: (usize, f32),
    drag: Option<[f32; 2]>,
    drawn: Option<(Camera, [f32; 2])>,
    paths: Vec<(f32, String, [u8; 3])>,
}
impl Default for Plot {
    fn default() -> Self {
        Self {
            mesh: Mesh::default(),
            camera: Camera::default(),
            scale: 10.0,
            channel: 1,
            source: None,
            built: (usize::MAX, 0.0),
            drag: None,
            drawn: None,
            paths: Vec::new(),
        }
    }
}
impl Plot {
    pub fn sync(&mut self, data: &ResultData) -> bool {
        let same = match (&self.source, &data.surface) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        };
        self.scale = if self.scale.is_finite() { self.scale.clamp(0.1, 20000.0) } else { 10.0 };
        if same && self.built == (self.channel, self.scale) {
            return false;
        }
        let first = self.source.is_none();
        self.source = data.surface.clone();
        self.built = (self.channel, self.scale);
        self.drawn = None;
        if let Some(source) = &self.source {
            self.mesh.build(source, self.channel, self.scale);
            if first {
                self.reset();
            }
        } else {
            self.mesh.faces.clear();
            self.mesh.points.clear();
        }
        true
    }
    pub fn reset(&mut self) {
        self.camera = Camera::default();
        self.camera.pitch = 0.70;
        self.camera.frame(self.mesh.bounds);
    }
    pub fn controls(&mut self, ui: &mut Ui, at: Option<[f32; 2]>) {
        if let Some([x, y]) = at {
            for i in 0..4 {
                ui.place([x + i as f32 * 108.0, y]);
                self.control(ui, i);
            }
        } else {
            ui.columns(120.0, |ui| {
                for i in 0..4 {
                    ui.column(|ui| self.control(ui, i));
                }
            });
        }
    }
    fn control(&mut self, ui: &mut Ui, i: usize) {
        if ui.button(["Reset view", "Top view", "Zoom in", "Zoom out"][i]).clicked() {
            match i {
                0 => self.reset(),
                1 => {
                    self.reset();
                    self.camera.pitch = 1.55;
                }
                2 => self.camera.zoom(-180.0),
                _ => self.camera.zoom(180.0),
            }
        }
    }
    pub fn preview(&mut self, ui: &mut Ui, size: [f32; 2]) {
        let mut c = ui.canvas("3D measured surface / drag to orbit", size);
        c.grab();
        for event in c.pointer() {
            match *event {
                Pointer::Down(at) => self.drag = Some(at),
                Pointer::Move(at) => {
                    if let Some(last) = self.drag {
                        self.camera.orbit([at[0] - last[0], at[1] - last[1]]);
                        self.drag = Some(at);
                    }
                }
                Pointer::Up(_) => self.drag = None,
            }
        }
        for key in c.keys() {
            if let Key::Down(key) = key {
                match key.as_str() {
                    "Home" => self.reset(),
                    "Equal" | "NumpadAdd" => self.camera.zoom(-180.0),
                    "Minus" | "NumpadSubtract" => self.camera.zoom(180.0),
                    "ArrowLeft" => self.camera.orbit([-12., 0.]),
                    "ArrowRight" => self.camera.orbit([12., 0.]),
                    "ArrowUp" => self.camera.orbit([0., -12.]),
                    "ArrowDown" => self.camera.orbit([0., 12.]),
                    _ => {}
                }
            }
        }
        c.rect([0., 0.], size, [18, 18, 18], true);
        let area = [size[0] - 80.0, size[1] - 28.0];
        let m = self.camera.matrix(area[0] / area[1]);
        let project = |p: [f32; 3]| -> [f32; 3] {
            let q: [f32; 4] = std::array::from_fn(|r| m[0][r] * p[0] + m[1][r] * p[1] + m[2][r] * p[2] + m[3][r]);
            [(q[0] / q[3] * 0.5 + 0.5) * area[0], (-q[1] / q[3] * 0.5 + 0.5) * area[1] + 12., q[2] / q[3]]
        };
        if self.drawn != Some((self.camera, size)) {
            self.paths.resize_with(self.mesh.faces.len() / 3, || (0., String::with_capacity(100), [0; 3]));
            let light = self.camera.light();
            for (path, tri) in self.paths.iter_mut().zip(self.mesh.faces.as_chunks::<3>().0) {
                let points = tri.map(|v| project([v[0], v[1], v[2]]));
                path.0 = points.iter().map(|p| p[2]).sum::<f32>() / 3.0;
                path.1.clear();
                let shade = 0.60 + 0.40 * (tri[0][3] * light[0] + tri[0][4] * light[1] + tri[0][5] * light[2]).abs();
                path.2 = std::array::from_fn(|k| {
                    (tri.iter().map(|v| v[k + 6]).sum::<f32>() / 3.0 * shade * 255.).clamp(0., 255.) as u8
                });
                let [a, b, d] = points;
                if points.iter().all(|p| p[2] > 0. && p[2] < 1. && p[0].is_finite() && p[1].is_finite()) {
                    let _ = write!(
                        path.1,
                        "M {:.2} {:.2} L {:.2} {:.2} L {:.2} {:.2} Z",
                        a[0], a[1], b[0], b[1], d[0], d[1]
                    );
                }
            }
            self.paths.sort_unstable_by(|a, b| b.0.total_cmp(&a.0));
            self.drawn = Some((self.camera, size));
        }
        if let Some(source) = &self.source {
            let [lo, hi] = self.mesh.bounds;
            for i in 0..=4 {
                let t = i as f32 / 4.;
                let x = lo[0] + (hi[0] - lo[0]) * t;
                let y = lo[1] + (hi[1] - lo[1]) * t;
                for (a, b) in [([x, lo[1], lo[2]], [x, hi[1], lo[2]]), ([lo[0], y, lo[2]], [hi[0], y, lo[2]])] {
                    let a = project(a);
                    let b = project(b);
                    c.line([a[0], a[1]], [b[0], b[1]], [53, 53, 53]);
                }
            }
            for (_, path, color) in &self.paths {
                if !path.is_empty() {
                    c.path(path, *color, true);
                }
            }
            if self.mesh.faces.is_empty() {
                for p in &self.mesh.points {
                    let p = project(*p);
                    c.circle([p[0], p[1]], 2.5, [227, 184, 105], true);
                }
            }
            let [low, high] = source.range(self.channel);
            for i in 0..80 {
                c.rect(
                    [size[0] - 66., 30. + i as f32 * (size[1] - 90.) / 80.],
                    [12., (size[1] - 90.) / 80. + 0.5],
                    color(1. - i as f32 / 79.),
                    true,
                );
            }
            c.text([size[0] - 68., 18.], INK, "um");
            c.text([size[0] - 48., 40.], INK, format_args!("{high:.1}"));
            c.text([size[0] - 48., size[1] - 60.], INK, format_args!("{low:.1}"));
            c.text(
                [12., size[1] - 10.],
                MUTED,
                format_args!("X / Y: {:.1} x {:.1} mm   Z display: {}x", hi[0] - lo[0], hi[1] - lo[1], self.scale),
            );
        } else {
            c.text([16., 30.], MUTED, "Start a scan to build the measured surface.");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn placed_and_responsive_controls_move_the_same_camera() {
        let mut plot = Plot::default();
        plot.controls(&mut Ui::new(0, "3 1"), Some([16., 605.]));
        assert_eq!(plot.camera.pitch, 1.55);
        plot.controls(&mut Ui::new(0, "2 1"), None);
        assert_eq!(plot.camera.pitch, 0.70);
    }
    #[test]
    fn pointer_and_keys_rotate_zoom_and_reset_the_node() {
        let mut plot = Plot::default();
        let initial = plot.camera;
        let mut ui = Ui::new(0, "0 down 100 100\n0 move 140 120\n0 up 140 120");
        plot.preview(&mut ui, [588., 350.]);
        assert_ne!(plot.camera.yaw, initial.yaw);
        assert_ne!(plot.camera.pitch, initial.pitch);
        let distance = plot.camera.distance;
        plot.preview(&mut Ui::new(0, "0 key down Equal"), [588., 350.]);
        assert!(plot.camera.distance < distance);
        plot.preview(&mut Ui::new(0, "0 key down Home"), [588., 350.]);
        assert_eq!(plot.camera.yaw, Camera::default().yaw);
    }
}

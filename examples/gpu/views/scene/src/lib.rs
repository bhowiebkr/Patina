use patina_gpu::ViewportSurface;
use patina_sdk::{Graph, Pointer, Ui, view};

/// A viewport that the pointer colors (15.8). A drag sets the color, and the wheel sets the light.
#[view]
pub struct Scene {
    surface: ViewportSurface,
    color: [f64; 2],
    light: f64,
    error: String,
}

impl Scene {
    pub fn init() -> Self {
        Scene { surface: ViewportSurface::default(), color: [0.2, 0.4], light: 0.5, error: String::new() }
    }

    pub fn ui(&mut self, ui: &mut Ui, _graph: &mut Graph) {
        let viewport = ui.viewport("Scene", 320.0);
        let [w, h] = [viewport.rect[2].max(1) as f64, viewport.rect[3].max(1) as f64];
        for pointer in &viewport.pointer {
            if let Pointer::Down([x, y]) | Pointer::Move([x, y]) = pointer {
                self.color = [(*x as f64 / w).clamp(0.0, 1.0), (*y as f64 / h).clamp(0.0, 1.0)];
            }
        }
        self.light = (self.light - viewport.wheel as f64 / 1000.0).clamp(0.0, 1.0);
        let [r, g, b] = [self.color[0], self.color[1], 1.0 - self.color[0]].map(|c| c * self.light);
        self.error = self.surface.draw(&viewport, patina_gpu::wgpu::Color { r, g, b, a: 1.0 }, |_, _| {}).err().unwrap_or_default();
        ui.label(&format!("Drag to change the color. Turn the wheel to change the light: {:.0} %.", self.light * 100.0));
        if !self.error.is_empty() {
            ui.label(&self.error);
        }
    }
}

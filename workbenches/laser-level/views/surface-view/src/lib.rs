use measurement::{ResultData, surface::color};
use measurement_ui::surface_render as render;
use measurement_ui::{INK, surface::Plot};
use patina_sdk::{Button, Graph, Key, Pointer, Ui};

#[patina_sdk::view]
pub struct SurfaceView {
    plot: Plot,
    renderer: Option<render::Renderer>,
    stale: bool,
    drag: Option<([f32; 2], Button)>,
}
impl SurfaceView {
    pub fn init() -> Self {
        Self { plot: Plot::default(), renderer: None, stale: true, drag: None }
    }
    pub fn ui(&mut self, ui: &mut Ui, graph: &mut Graph) {
        ui.label("SURFACE / MEASURED TOPOGRAPHY");
        let id = graph.nodes().find(|id| graph.folder(id) == Some("nodes/results"));
        let data = id.and_then(|id| graph.value::<ResultData>(id, "out")).cloned().unwrap_or_default();
        ui.tabs("Height channel", &mut self.plot.channel, &["Leveled", "Measured height", "Scrape", "Shim"]);
        ui.number_unit("Z display scale", &mut self.plot.scale, 1.0, "x");
        self.stale |= self.plot.sync(&data);
        self.plot.controls(ui, None);
        let size = ui.available_size().unwrap_or([800., 650.]);
        let viewport = ui.viewport("Measured surface / orbit, pan and zoom", (size[1] - 230.).clamp(180., 3000.));
        for p in &viewport.pointer {
            match *p {
                Pointer::Down(at) => {
                    let press = viewport.press.unwrap_or_default();
                    self.drag = Some((at, if press.shift { Button::Middle } else { press.button }));
                }
                Pointer::Move(at) => {
                    if let Some((last, button)) = self.drag {
                        let d = [at[0] - last[0], at[1] - last[1]];
                        match button {
                            Button::Left => self.plot.camera.orbit(d),
                            Button::Middle => self.plot.camera.pan(d, viewport.rect[3] as f32),
                            Button::Right => self.plot.camera.zoom(-2.0 * (d[0] + d[1])),
                        }
                        self.drag = Some((at, button));
                    }
                }
                Pointer::Up(_) => self.drag = None,
            }
        }
        self.plot.camera.zoom(viewport.wheel);
        if viewport.keys.iter().any(|k| matches!(k,Key::Down(name) if name=="Home")) {
            self.plot.reset();
        }
        if let Some(s) = &data.surface {
            let [low, high] = s.range(self.plot.channel);
            let width = size[0].clamp(240., 2000.) - 32.;
            let mut c = ui.canvas("Height legend in micrometers", [width, 46.]);
            for i in 0..100 {
                c.rect([i as f32 * width / 100., 0.], [width / 100. + 0.5, 12.], color(i as f32 / 99.), true);
            }
            c.text([0., 32.], INK, format_args!("{low:.2} um"));
            c.text([width - 100., 32.], INK, format_args!("{high:.2} um"));
            drop(c);
            ui.label(&format!(
                "{} / {} measured stations. X / Y: {:.1} x {:.1} mm. Z display: {}x.",
                s.measured(),
                s.xy.len(),
                s.bounds[1][0] - s.bounds[0][0],
                s.bounds[1][1] - s.bounds[0][1],
                self.plot.scale
            ));
            if self.plot.channel != 1 && !s.leveled {
                ui.label("More stations are needed for a plane fit. Select Measured height.");
            }
        } else {
            ui.label("Start scan in Survey. This view shows saved measurements, not the simulation target.");
        }
        ui.label("Left drag: orbit. Middle or Shift drag: pan. Right drag or wheel: zoom. Home: reset.");
        if viewport.window == 0 {
            return;
        }
        let outcome = (|| -> Result<(), String> {
            if self.renderer.is_none() {
                self.renderer = Some(render::Renderer::new()?);
            }
            let r = self.renderer.as_mut().unwrap();
            if self.stale {
                r.upload(&self.plot.mesh)?;
                self.stale = false;
            }
            r.draw(&viewport, &self.plot.camera)
        })();
        if let Err(error) = outcome {
            ui.label(&error);
        }
    }
}

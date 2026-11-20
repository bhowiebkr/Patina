use material_lab::{Material, render::Look};
use patina_sdk::{Button, Graph, Key, Pointer, Ui};

/// A resizable PBR preview. The mouse works as in Maya: the left button orbits, the middle button
/// pans, and the right button or the wheel zooms. Home frames the material.
#[patina_sdk::view]
pub struct MaterialPreview {
    look: Option<Look>,
    /// The last pointer place of a drag and the button that started it.
    drag: Option<([f32; 2], Button)>,
    channel: usize,
    exposure: f32,
    rotation: f32,
}
impl MaterialPreview {
    pub fn init() -> Self {
        Self {
            look: None,
            drag: None,
            channel: 0,
            exposure: material_lab::recipe().manifest.exposure,
            rotation: material_lab::recipe().manifest.rotation.rem_euclid(360.0),
        }
    }
    pub fn ui(&mut self, ui: &mut Ui, graph: &mut Graph) {
        ui.label("MATERIAL / SURFACE PREVIEW");
        let source = graph
            .nodes()
            .find(|id| graph.folder(id) == Some("nodes/material-output"))
            .and_then(|id| {
                graph
                    .wires()
                    .find(|(_, to)| *to == format!("{id}.material"))
            })
            .and_then(|(from, _)| from.split_once('.'));
        let Some((id, port)) = source else {
            ui.label("Open the materials graph and connect its Output.");
            return;
        };
        let Some(material) = graph.value::<Material>(id, port) else {
            ui.label("Press Run with this view open to capture the material.");
            return;
        };
        let channels = [
            "Shaded",
            "Base color",
            "Roughness",
            "Metallic",
            "Height",
            "Occlusion",
            "Normal",
        ];
        let mut changed = ui
            .dropdown("Channel", &mut self.channel, &channels)
            .changed();
        let size = ui.available_size().unwrap_or([320.0, 520.0]);
        let viewport = ui.viewport(
            "Material preview",
            size[0].min(size[1] - 190.0).clamp(120.0, 1400.0),
        );
        changed |= ui
            .slider("Exposure", &mut self.exposure, 0.1..=4.0)
            .changed();
        changed |= ui
            .slider("Studio rotation", &mut self.rotation, 0.0..=360.0)
            .changed();
        let reset = ui.button("Frame material").clicked();
        ui.label("Left drag to orbit. Middle drag to pan. Right drag or wheel to zoom. Home to frame.");
        if self.look.is_none() {
            match Look::new([320, 320]) {
                Ok(look) => self.look = Some(look),
                Err(error) => {
                    ui.label(&error);
                    return;
                }
            }
        }
        let Some(look) = &mut self.look else {
            return;
        };
        for pointer in &viewport.pointer {
            match *pointer {
                Pointer::Down(at) => {
                    let press = viewport.press.unwrap_or_default();
                    let button = if press.shift { Button::Middle } else { press.button };
                    self.drag = Some((at, button));
                }
                Pointer::Move(at) => {
                    let Some((last, button)) = &mut self.drag else {
                        continue;
                    };
                    let [dx, dy] = [at[0] - last[0], at[1] - last[1]];
                    *last = at;
                    match button {
                        Button::Left => look.camera.orbit(dx * 0.008, dy * 0.008),
                        Button::Middle => look.camera.pan(dx, dy, viewport.rect[3] as f32),
                        // As in Maya, a drag to the right or down moves in, and to the left or up moves out.
                        Button::Right => look.camera.zoom((-0.002 * (dx + dy)).clamp(-1.0, 1.0).exp()),
                    }
                    look.dirty = true;
                }
                Pointer::Up(_) => self.drag = None,
            }
        }
        if viewport.wheel != 0.0 {
            look.camera
                .zoom((viewport.wheel * 0.001).clamp(-1.0, 1.0).exp());
            look.dirty = true;
        }
        if (reset
            || viewport
                .keys
                .iter()
                .any(|key| matches!(key, Key::Down(name) if name == "Home")))
            && let Some(image) = &material.0
        {
            look.camera = image.target.0.camera;
            look.dirty = true;
        }
        if changed || look.light.channel != self.channel as u32 {
            look.light.channel = self.channel as u32;
            look.light.exposure = self.exposure;
            look.light.env_yaw = self.rotation / 360.0;
            look.dirty = true;
        }
        if let Err(error) = look.draw(&viewport, material) {
            ui.label(&error);
        }
    }
}

//! The 3D view of the model workbench. It shows the solid of the selected node next to the graph.

mod camera;
mod render;

use camera::Camera;
use geometry::{Mesh, Solid};
use patina_sdk::{Graph, Key, Pointer, Ui};
use render::{Options, Renderer};

/// Shows the solid of the selected node. Drag to orbit, Shift and drag to pan, use the wheel to
/// zoom, or press Home to frame.
#[patina_sdk::view]
pub struct ModelView {
    renderer: Option<Renderer>,
    camera: Camera,
    options: Options,
    /// The output that the view shows, as `<node>.<output>`. It stays when the selection has no solid.
    source: String,
    shown: Option<Shown>,
    /// True when the GPU does not have the mesh of `shown` with the current part colors.
    stale: bool,
    drag: Option<[f32; 2]>,
    shift: bool,
}

/// The solid that the view shows.
struct Shown {
    serial: u64,
    text: String,
    bounds: [[f32; 3]; 2],
    mesh: Mesh,
}

impl ModelView {
    pub fn init() -> Self {
        Self {
            renderer: None,
            camera: Camera::default(),
            options: Options { grid: true, wireframe: false, parts: true },
            source: String::new(),
            shown: None,
            stale: true,
            drag: None,
            shift: false,
        }
    }

    pub fn ui(&mut self, ui: &mut Ui, graph: &mut Graph) {
        ui.label("MODEL / 3D VIEW");
        let found = graph.selected().find_map(|id| solid_of(graph, id)).or_else(|| {
            let from = graph.wires().map(|(from, _)| from).find(|from| *from == self.source)?;
            let (id, key) = from.split_once('.')?;
            Some((from, graph.value::<Solid>(id, key)?))
        });
        let mut reframe = false;
        if let Some((from, solid)) = found {
            if from != self.source {
                self.source.clear();
                self.source.push_str(from);
            }
            if self.shown.as_ref().is_none_or(|shown| shown.serial != solid.serial()) {
                let bounds = solid.bounds().map(|corner| corner.map(|v| v as f32));
                reframe = self.shown.is_none() && bounds.iter().flatten().all(|v| v.is_finite());
                self.shown = Some(Shown { serial: solid.serial(), text: format!("{solid:?}"), bounds, mesh: solid.mesh() });
                self.stale = true;
            }
        }
        let node = self.source.split_once('.').map_or("", |(id, _)| id);
        match &self.shown {
            Some(shown) => {
                let name = graph.folder(node).map_or(node, |f| f.trim_start_matches("nodes/"));
                ui.label(&format!("{name} - {}", shown.text));
            }
            None => ui.label("Select a model node. The view shows its output when the output has a wire."),
        }
        if let Some(error) = graph.error(node) {
            ui.label(error);
        }
        let size = ui.available_size().unwrap_or([480.0, 480.0]);
        let viewport = ui.viewport("3D view", (size[1] - 200.0).clamp(160.0, 4000.0));
        let parts = self.options.parts;
        ui.columns(110.0, |ui| {
            ui.column(|ui| _ = ui.toggle("Wireframe", &mut self.options.wireframe));
            ui.column(|ui| _ = ui.toggle("Grid", &mut self.options.grid));
            ui.column(|ui| _ = ui.toggle("Part colors", &mut self.options.parts));
        });
        // The part colors are in the vertices, so a change of the toggle sends the mesh again.
        self.stale |= parts != self.options.parts;
        reframe |= ui.button("Frame model").clicked();
        ui.label("Drag to orbit. Shift and drag to pan. Wheel to zoom. Home to frame.");

        for key in &viewport.keys {
            match key {
                Key::Down(name) | Key::Up(name) if name.starts_with("Shift") => self.shift = matches!(key, Key::Down(_)),
                Key::Down(name) if name == "Home" => reframe = true,
                _ => {}
            }
        }
        for pointer in &viewport.pointer {
            match *pointer {
                Pointer::Down(at) => self.drag = Some(at),
                Pointer::Move(at) => {
                    if let Some(last) = self.drag.replace(at) {
                        let delta = [at[0] - last[0], at[1] - last[1]];
                        if self.shift {
                            self.camera.pan(delta, viewport.rect[3] as f32);
                        } else {
                            self.camera.orbit(delta);
                        }
                    }
                }
                Pointer::Up(_) => self.drag = None,
            }
        }
        if viewport.wheel != 0.0 {
            self.camera.zoom(viewport.wheel);
        }
        if reframe && let Some(shown) = &self.shown {
            self.camera.frame(shown.bounds);
        }
        // With no solid, the view shows the grid alone.
        let result = renderer(&mut self.renderer).and_then(|r| {
            if self.stale {
                r.upload(self.shown.as_ref().map_or(&Mesh::default(), |shown| &shown.mesh), self.options.parts)?;
                self.stale = false;
            }
            r.draw(&viewport, &self.camera, self.options)
        });
        if let Err(error) = result {
            ui.label(&error);
        }
    }
}

/// The renderer of the view. It starts at the first draw.
fn renderer(slot: &mut Option<Renderer>) -> Result<&mut Renderer, String> {
    match slot {
        Some(renderer) => Ok(renderer),
        None => Ok(slot.insert(Renderer::new()?)),
    }
}

/// The solid of node `id` and the output that gives it. It is the first solid output of the node
/// with a wire, or for a node with no such output, the first solid that goes into the node.
fn solid_of<'g>(graph: &'g Graph, id: &str) -> Option<(&'g str, &'g Solid)> {
    let on = |pin: &str| pin.split_once('.').is_some_and(|(n, _)| n == id);
    let outputs = graph.wires().any(|(from, _)| on(from));
    graph.wires().filter(|&(from, to)| on(if outputs { from } else { to })).find_map(|(from, _)| {
        let (n, key) = from.split_once('.')?;
        Some((from, graph.value::<Solid>(n, key)?))
    })
}

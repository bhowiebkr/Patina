//! The 3D view of the model workbench. It shows the solid or the scene of the selected node next
//! to the graph.

mod camera;
mod render;

use camera::Camera;
use geometry::{Mesh, Scene, Solid};
use patina_sdk::{Button, Graph, Key, Pointer, Press, Ui};
use render::{Content, Options, Renderer};

/// Shows the solid or the scene of the selected node. The mouse works as in Maya: the left button
/// orbits, the middle button pans, and the right button or the wheel zooms. Home frames the model.
#[patina_sdk::view]
pub struct ModelView {
    renderer: Option<Renderer>,
    camera: Camera,
    options: Options,
    /// The output that the view shows, as `<node>.<output>`. It stays when the selection has no model.
    source: String,
    shown: Option<Shown>,
    /// True when the GPU does not have the mesh of `shown` with the current part colors.
    stale: bool,
    drag: Option<([f32; 2], Gesture)>,
}

/// What a drag in the view does, as in Maya. Alt and a button does the same as the button alone.
#[derive(Clone, Copy)]
enum Gesture {
    Orbit,
    Pan,
    Dolly,
}

impl Gesture {
    /// The left button orbits, or pans with Shift. The middle button pans. The right button zooms.
    fn of(press: Press) -> Gesture {
        match press.button {
            Button::Left if press.shift => Gesture::Pan,
            Button::Left => Gesture::Orbit,
            Button::Middle => Gesture::Pan,
            Button::Right => Gesture::Dolly,
        }
    }
}

/// The model that the view shows.
struct Shown {
    serial: u64,
    text: String,
    bounds: [[f32; 3]; 2],
    content: Content,
}

/// A value that the view can show.
enum Model<'g> {
    Solid(&'g Solid),
    Scene(&'g Scene),
}

impl Model<'_> {
    fn serial(&self) -> u64 {
        match self {
            Model::Solid(solid) => solid.serial(),
            Model::Scene(scene) => scene.serial(),
        }
    }

    fn shown(&self) -> Shown {
        let (text, bounds, content) = match self {
            Model::Solid(solid) => (format!("{solid:?}"), solid.bounds(), Content::Mesh(solid.mesh())),
            Model::Scene(scene) => (format!("{scene:?}"), scene.bounds(), Content::Scene((*scene).clone())),
        };
        Shown { serial: self.serial(), text, bounds: bounds.map(|corner| corner.map(|v| v as f32)), content }
    }
}

impl ModelView {
    pub fn init() -> Self {
        Self {
            renderer: None,
            camera: Camera::default(),
            options: Options { grid: true, wireframe: false, parts: true, toon: false },
            source: String::new(),
            shown: None,
            stale: true,
            drag: None,
        }
    }

    pub fn ui(&mut self, ui: &mut Ui, graph: &mut Graph) {
        ui.label("MODEL / 3D VIEW");
        let picked = graph.selected().find_map(|id| model_of(graph, id));
        if let Some(((id, key), _)) = picked
            && self.source.split_once('.') != Some((id, key))
        {
            self.source.clear();
            self.source.extend([id, ".", key]);
        }
        let found = picked.map(|(_, model)| model).or_else(|| {
            let (id, key) = self.source.split_once('.')?;
            model(graph, id, key)
        });
        let mut reframe = false;
        if let Some(model) = found
            && self.shown.as_ref().is_none_or(|shown| shown.serial != model.serial())
        {
            let shown = model.shown();
            reframe = self.shown.is_none() && shown.bounds.iter().flatten().all(|v| v.is_finite());
            self.shown = Some(shown);
            self.stale = true;
        }
        let node = self.source.split_once('.').map_or("", |(id, _)| id);
        match &self.shown {
            Some(shown) => {
                let name = graph.folder(node).map_or(node, |f| f.trim_start_matches("nodes/"));
                ui.label(&format!("{name} - {}", shown.text));
            }
            None => ui.label("Select a model node. The view shows its output after the node runs."),
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
            ui.column(|ui| _ = ui.toggle("Toon", &mut self.options.toon));
        });
        // The part colors are in the vertices, so a change of the toggle sends the mesh again.
        self.stale |= parts != self.options.parts;
        reframe |= ui.button("Frame model").clicked();
        ui.label("Left drag to orbit. Middle drag to pan. Right drag or wheel to zoom. Home to frame.");

        reframe |= viewport.keys.iter().any(|key| matches!(key, Key::Down(name) if name == "Home"));
        for pointer in &viewport.pointer {
            match *pointer {
                Pointer::Down(at) => self.drag = Some((at, Gesture::of(viewport.press.unwrap_or_default()))),
                Pointer::Move(at) => {
                    let Some((last, gesture)) = &mut self.drag else { continue };
                    let [dx, dy] = [at[0] - last[0], at[1] - last[1]];
                    *last = at;
                    match gesture {
                        Gesture::Orbit => self.camera.orbit([dx, dy]),
                        Gesture::Pan => self.camera.pan([dx, dy], viewport.rect[3] as f32),
                        // As in Maya, a drag to the right or down moves in, and to the left or up moves out.
                        Gesture::Dolly => self.camera.zoom(-2.0 * (dx + dy)),
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
        // With no model, the view shows the grid alone.
        let result = renderer(&mut self.renderer).and_then(|r| {
            if self.stale {
                r.upload(self.shown.as_ref().map_or(&Content::Mesh(Mesh::default()), |shown| &shown.content), self.options.parts)?;
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

/// The model of node `id` and the output that gives it, as (node, output). It is the first model
/// output of the node, or for a node with no model output, the first model that goes into the node.
fn model_of<'g>(graph: &'g Graph, id: &'g str) -> Option<((&'g str, &'g str), Model<'g>)> {
    let own = graph.outputs(id).find_map(|key| Some(((id, key), model(graph, id, key)?)));
    let into = || graph.wires().filter(|(_, to)| to.split_once('.').is_some_and(|(n, _)| n == id)).filter_map(|(from, _)| from.split_once('.'));
    own.or_else(|| into().find_map(|(n, key)| Some(((n, key), model(graph, n, key)?))))
}

/// The solid or the scene at the output `key` of node `id`.
fn model<'g>(graph: &'g Graph, id: &str, key: &str) -> Option<Model<'g>> {
    graph.value::<Solid>(id, key).map(Model::Solid).or_else(|| graph.value::<Scene>(id, key).map(Model::Scene))
}

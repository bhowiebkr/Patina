use crate::{
    parameters::CONSOLE,
    presets::{Preset, factory},
    title,
};
use patina_sdk::{Graph, Ui, project_path};
use std::{collections::BTreeSet, sync::Arc};
use synth_dsp::audition::{Preview, outputs};

pub struct Sounds {
    presets: Vec<Preset>,
    search: String,
    category: usize,
    filter: usize,
    row: Option<usize>,
    selected: usize,
    favorites: BTreeSet<String>,
    recent: BTreeSet<String>,
    baseline: Option<Preset>,
    current: String,
    name: String,
    pending: Option<usize>,
    preview: Option<Arc<Preview>>,
    level: f32,
    status: String,
}
impl Default for Sounds {
    fn default() -> Self {
        Self {
            presets: Vec::new(),
            search: String::new(),
            category: 0,
            filter: 0,
            row: Some(0),
            selected: 0,
            favorites: BTreeSet::new(),
            recent: BTreeSet::new(),
            baseline: None,
            current: "Current patch".into(),
            name: "My sound".into(),
            pending: None,
            preview: None,
            level: 0.15,
            status: String::new(),
        }
    }
}
impl Drop for Sounds {
    fn drop(&mut self) {
        self.stop();
    }
}
impl Sounds {
    fn stop(&mut self) {
        if let Some(preview) = self.preview.take() {
            preview.stop();
        }
    }
    fn save(&mut self, graph: &Graph) -> bool {
        let preset = Preset::capture(graph, &self.name);
        match preset.save(&project_path("sounds")) {
            Ok(()) => {
                self.current = preset.name.clone();
                self.baseline = Some(preset.clone());
                self.presets.push(preset);
                self.status = "Saved sound. The graph saves separately with Save.".into();
                true
            }
            Err(e) => {
                self.status = e;
                false
            }
        }
    }
    fn load(&mut self, index: usize, graph: &mut Graph) {
        self.stop();
        let preset = &self.presets[index];
        match preset.apply(graph) {
            Ok(()) => {
                self.current = preset.name.clone();
                self.baseline = Some(preset.clone());
                self.recent.insert(preset.name.clone());
                self.status="Loaded. Undo restores the previous settings. Select Phrase on the console to hear it.".into();
            }
            Err(e) => self.status = e,
        }
        self.pending = None;
    }
    pub fn ui(&mut self, ui: &mut Ui, graph: &mut Graph) {
        if self
            .preview
            .as_ref()
            .is_some_and(|p| !p.active.load(std::sync::atomic::Ordering::Relaxed))
        {
            self.stop();
        }
        title(ui, "EMBER / SOUNDS", "Phrase sound presets");
        if graph.folder(CONSOLE) != Some("nodes/console") {
            self.stop();
            ui.label("Open Ember's main graph to browse its sounds.");
            return;
        }
        if self.baseline.is_none() {
            let base = Preset::capture(graph, "Current patch");
            self.presets = factory(graph);
            if let Ok(entries) = std::fs::read_dir(project_path("sounds")) {
                for entry in entries.flatten().take(256) {
                    if entry.path().extension().is_some_and(|e| e == "ember") {
                        match Preset::read(&entry.path(), &base) {
                            Ok(p) => self.presets.push(p),
                            Err(e) => {
                                self.status =
                                    format!("Skipped {}: {e}", entry.file_name().to_string_lossy())
                            }
                        }
                    }
                }
            }
            self.baseline = Some(base);
        }
        let modified = self
            .baseline
            .as_ref()
            .is_some_and(|b| b.values != Preset::capture(graph, "").values);
        ui.label(&format!(
            "{}{}",
            self.current,
            if modified { " / Modified" } else { "" }
        ));
        if ui.text_field("Search", &mut self.search).changed() {
            self.row = None;
        }
        let categories = ["All", "Bass", "Lead", "Keys", "Pad", "User"];
        if ui
            .dropdown("Category", &mut self.category, &categories)
            .changed()
        {
            self.row = None;
        }
        if ui
            .tabs(
                "Collection",
                &mut self.filter,
                &["All", "Favorites", "Recent"],
            )
            .changed()
        {
            self.row = None;
        }
        let ids: Vec<_> = self
            .presets
            .iter()
            .enumerate()
            .filter(|(_, p)| {
                p.name.to_lowercase().contains(&self.search.to_lowercase())
                    && (self.category == 0 || p.category == categories[self.category])
                    && match self.filter {
                        1 => self.favorites.contains(&p.name),
                        2 => self.recent.contains(&p.name),
                        _ => true,
                    }
            })
            .map(|(i, _)| i)
            .collect();
        let names: Vec<_> = ids.iter().map(|&i| self.presets[i].name.as_str()).collect();
        if ui
            .table("Sound library", &["Sound"], &names, &mut self.row)
            .changed()
            && let Some(index) = self.row.and_then(|r| ids.get(r)).copied()
        {
            self.stop();
            self.selected = index;
        }
        let preset = &self.presets[self.selected];
        ui.label(&format!("{} / {}", preset.name, preset.category));
        let mut star = self.favorites.contains(&preset.name);
        if ui.toggle("Favorite this session", &mut star).changed() {
            if star {
                self.favorites.insert(preset.name.clone());
            } else {
                self.favorites.remove(&preset.name);
            }
        }
        if ui
            .slider("Preview level", &mut self.level, 0.0..=0.5)
            .changed()
            && let Some(p) = &self.preview
        {
            p.level(self.level);
        }
        if ui
            .button(if self.preview.is_some() {
                "Stop preview"
            } else {
                "Preview sound"
            })
            .clicked()
        {
            if self.preview.is_some() {
                self.stop();
            } else {
                let mut available = outputs();
                if available.len() == 1 {
                    let preview = available.pop().unwrap();
                    preview.start(self.presets[self.selected].audition(), self.level);
                    self.preview = Some(preview);
                    self.status =
                        "Audition uses a separate C minor phrase. The saved patch stays unchanged."
                            .into();
                } else {
                    self.status="Run a patch with one Ember output to preview. Device output must be enabled.".into();
                }
            }
        }
        if let Some(p) = &self.preview {
            p.heartbeat();
        }
        ui.label("Preview needs engine Run and device output enabled.");
        if ui.button("Load sound").clicked() {
            if modified {
                self.pending = Some(self.selected);
            } else {
                self.load(self.selected, graph);
            }
        }
        ui.text_field("Save as name", &mut self.name);
        if ui.button("Save sound as new").clicked() {
            self.save(graph);
        }
        if let Some(index) = self.pending {
            ui.label("The current sound has changes. Save or discard them before loading.");
            if ui.button("Save as, then load").clicked() && self.save(graph) {
                self.load(index, graph);
            }
            if ui.button("Discard sound changes and load").clicked() {
                self.load(index, graph);
            }
            if ui.button("Cancel load").clicked() {
                self.pending = None;
            }
        }
        if !self.status.is_empty() {
            ui.label(&self.status);
        }
    }
}

use crate::{parameter, parameters::*, title};
use patina_sdk::{Graph, Ui};

pub struct Controls {
    pins: Vec<usize>,
    choice: usize,
}

impl Default for Controls {
    fn default() -> Self {
        Self {
            pins: vec![0, 1, 4, 5, 15, 16],
            choice: 0,
        }
    }
}

pub fn all() -> [Parameter; 17] {
    [
        MASTER,
        DRUMS,
        BPM,
        SWING,
        CUTOFF,
        RESONANCE,
        TONE[0],
        TONE[1],
        TONE[2],
        TONE[3],
        ENVELOPE[0],
        ENVELOPE[1],
        ENVELOPE[2],
        ENVELOPE[3],
        ECHO[0],
        ECHO[1],
        ECHO[2],
    ]
}

impl Controls {
    pub fn ui(&mut self, ui: &mut Ui, graph: &mut Graph) {
        title(ui, "EMBER / CONTROLS", "Keep useful parameters close");
        let items = all();
        let names: Vec<_> = items.iter().map(|p| p.label).collect();
        ui.dropdown("Parameter", &mut self.choice, &names);
        if ui.button("Pin parameter").clicked() && !self.pins.contains(&self.choice) {
            self.pins.push(self.choice);
        }
        let mut remove = None;
        for (slot, &index) in self.pins.iter().enumerate() {
            let p = items[index];
            parameter(ui, graph, p);
            ui.columns(150.0, |ui| {
                ui.column(|ui| {
                    if ui.button(&format!("Locate {}", p.label)).clicked() {
                        graph.select([p.node]);
                    }
                });
                ui.column(|ui| {
                    if ui.button(&format!("Unpin {}", p.label)).clicked() {
                        remove = Some(slot);
                    }
                });
            });
        }
        if let Some(index) = remove {
            self.pins.remove(index);
        }
        ui.label("Pins belong to this tab. Sound values save with the graph.");
    }
}

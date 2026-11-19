use patina_sdk::{Graph, Ui, view};

#[view]
pub struct Modulation {
    tool: workbench::Modulation,
}

impl Modulation {
    pub fn init() -> Self {
        Self {
            tool: Default::default(),
        }
    }
    pub fn ui(&mut self, ui: &mut Ui, graph: &mut Graph) {
        self.tool.ui(ui, graph);
    }
}

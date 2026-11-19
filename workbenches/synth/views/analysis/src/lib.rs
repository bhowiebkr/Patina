use patina_sdk::{Graph, Ui, view};

#[view]
pub struct Analysis {
    tool: workbench::Analysis,
}

impl Analysis {
    pub fn init() -> Self {
        Self {
            tool: Default::default(),
        }
    }
    pub fn ui(&mut self, ui: &mut Ui, graph: &mut Graph) {
        self.tool.ui(ui, graph);
    }
}

use patina_sdk::{Graph, Ui, view};

#[view]
pub struct Patterns {
    tool: workbench::Patterns,
}

impl Patterns {
    pub fn init() -> Self {
        Self {
            tool: Default::default(),
        }
    }
    pub fn ui(&mut self, ui: &mut Ui, graph: &mut Graph) {
        self.tool.ui(ui, graph);
    }
}

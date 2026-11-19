use patina_sdk::{Graph, Ui, view};

#[view]
pub struct Controls {
    tool: workbench::Controls,
}

impl Controls {
    pub fn init() -> Self {
        Self {
            tool: Default::default(),
        }
    }
    pub fn ui(&mut self, ui: &mut Ui, graph: &mut Graph) {
        self.tool.ui(ui, graph);
    }
}

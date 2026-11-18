use patina_sdk::{Pointer, Ui, node};

const RIM: [u8; 3] = [102, 102, 97];
const CAP: [u8; 3] = [101, 82, 25];
const LIT: [u8; 3] = [223, 166, 75];

/// A round push button. The output is on while the user holds the button down. The node draws
/// its whole body and puts its pin on the right edge of the circle (free layout, spec 4.3).
#[node(domain = frame, color = "#655219", free_layout)]
pub struct Button {
    held: bool,
}

impl Button {
    pub fn init() -> Self {
        Button { held: false }
    }

    pub fn process(&mut self) -> bool {
        self.held
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let mut canvas = ui.canvas("", [56.0, 56.0]);
        for p in canvas.pointer() {
            match p {
                Pointer::Down(_) => self.held = true,
                Pointer::Up(_) => self.held = false,
                Pointer::Move(_) => {}
            }
        }
        canvas.circle([28.0, 28.0], 26.0, RIM, true);
        canvas.circle([28.0, 28.0], 20.0, if self.held { LIT } else { CAP }, true);
        ui.pin("out", [56.0, 28.0]);
    }
}

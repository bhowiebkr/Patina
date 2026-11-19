use parts::{BODY, GLASS, LIT, METAL, board, round_rect};
use patina_sdk::{Pointer, Ui, node};

/// The cap of the button when it is up.
const CAP: [u8; 3] = [101, 82, 25];

/// A push button. The output is on while the user holds the button down. The node draws its whole
/// body and puts its pin on the right edge (free layout, spec 4.3).
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
        let mut c = ui.canvas(if self.held { "Button held" } else { "Button" }, [56.0, 56.0]);
        // A press on the cap acts at once, as the button acts while it is held. A drag on the board moves the node (4.3).
        c.grab_rect([15.0, 15.0], [26.0, 26.0]);
        for p in c.pointer() {
            match p {
                Pointer::Down(_) => self.held = true,
                Pointer::Up(_) => self.held = false,
                Pointer::Move(_) => {}
            }
        }
        board(&mut c, [56.0, 56.0]);
        // A tactile switch: a square body with a screw in each corner and a round cap.
        round_rect(&mut c, [10.0, 10.0], [36.0, 36.0], 4.0, BODY);
        for at in [[14.5, 14.5], [41.5, 14.5], [14.5, 41.5], [41.5, 41.5]] {
            c.circle(at, 2.0, METAL, true);
        }
        c.circle([28.0, 28.0], 12.5, GLASS, true);
        c.circle([28.0, 28.0], if self.held { 10.0 } else { 11.0 }, if self.held { LIT } else { CAP }, true);
        drop(c);
        ui.pin("out", [56.0, 28.0]);
    }
}

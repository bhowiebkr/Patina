//! The GUI of an audio node (spec 4.3). The GUI never gets the node state. The audio thread gives
//! a copy of the display value after each block, and the GUI draws with the latest copy.

use std::panic::AssertUnwindSafe;

use super::{Reporter, State, Triple, catch_quiet, draw, finish_state};

/// The GUI state `G` of one audio node and the latest display value `D` of the node. The audio
/// thread writes the value and the GUI thread reads it, with no lock (9.3).
pub struct Shown<G, D> {
    latest: Triple<D>,
    gui: State<(G, Option<D>)>,
}

impl<G, D> Shown<G, D> {
    #[allow(clippy::new_without_default, reason = "the generated code calls only `new`")]
    pub fn new() -> Shown<G, D> {
        Shown { latest: Triple::new(), gui: State::new() }
    }

    /// Gives a copy of `value` to the GUI. Only the task of the node calls it. A copy does not
    /// allocate or drop on the audio thread.
    pub fn publish(&self, value: D)
    where
        D: Copy,
    {
        self.latest.publish(&Some(value));
    }
}

/// Calls `display` of an audio node after its block and gives the value to the GUI. A panic drops
/// the state, as in `process` (4.7).
pub fn show<T, G, D: Copy>(shown: &Shown<G, D>, state: &mut Option<T>, display: impl Fn(&T) -> D, rep: &Reporter, node: u32) -> bool {
    let Some(value) = state.as_ref() else { return true };
    match catch_quiet(AssertUnwindSafe(|| display(value))) {
        Ok(value) => {
            shown.publish(value);
            true
        }
        Err(e) => finish_state(Err(e), state, rep, node),
    }
}

/// Calls the `ui` function of an audio node with its GUI state and the latest display value, as
/// `draw` does. Before the first block, the value is `D::default()`.
pub fn draw_shown<G: Default, D: Default>(shown: &Shown<G, D>, ui: impl Fn(&mut G, &D, &mut crate::Ui), cmd: (u32, &str), rep: &Reporter, node: u32) -> bool {
    let gui = |st: &mut (G, Option<D>), out: &mut crate::Ui| {
        let (gui, value) = st;
        shown.latest.fetch(value);
        ui(gui, value.get_or_insert_with(D::default), out);
    };
    draw(&shown.gui, || (G::default(), None), gui, cmd, rep, node)
}

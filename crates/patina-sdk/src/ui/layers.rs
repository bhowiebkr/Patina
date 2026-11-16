//! The layer stack widget of a node GUI (spec 4.3).

use super::{Response, Ui, push_clean, push_color};
use std::fmt::Write;

/// One row of a layer stack. `Ui::layer_stack` edits it.
#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub name: String,
    pub color: [u8; 3],
    pub visible: bool,
    /// The amount from 0 to 1. A layer without an amount, for example a base, has no amount field.
    pub amount: Option<f32>,
}

enum LayerEdit {
    Select(usize),
    Visible(usize, bool),
    Amount(usize, f32),
    Move(usize, usize),
}

/// Reads a layer event, for example `select 1`, `visible 0 0`, `amount 2 0.5` or `move 2 0`.
fn layer_edit(event: &str) -> Option<LayerEdit> {
    let mut words = event.split(' ');
    let kind = words.next()?;
    let i = words.next()?.parse::<usize>().ok()?;
    let word = words.next();
    match kind {
        "select" => Some(LayerEdit::Select(i)),
        "visible" => match word? {
            "0" => Some(LayerEdit::Visible(i, false)),
            "1" => Some(LayerEdit::Visible(i, true)),
            _ => None,
        },
        "amount" => word?.parse::<f32>().ok().filter(|v| v.is_finite()).map(|v| LayerEdit::Amount(i, v.clamp(0.0, 1.0))),
        "move" => word?.parse::<usize>().ok().map(|to| LayerEdit::Move(i, to)),
        _ => None,
    }
}

impl Ui {
    /// Shows a layer stack, with the first layer at the top. Each row has a handle to move the
    /// layer, a visibility toggle, a swatch, the name and the amount. `selected` is the selected
    /// layer. Show its properties below the stack, for example in a section.
    pub fn layer_stack(&mut self, text: &str, layers: &mut [Layer], selected: &mut usize) -> Response {
        let edit = self.push("layers", text).and_then(layer_edit);
        let _ = write!(self.tree, "\t{}", (*selected).min(layers.len().saturating_sub(1)));
        for layer in layers.iter() {
            let _ = write!(self.tree, "\t{} ", u8::from(layer.visible));
            match layer.amount {
                Some(amount) => _ = write!(self.tree, "{amount} "),
                None => self.tree.push_str("- "),
            }
            push_color(&mut self.tree, layer.color);
            self.tree.push(' ');
            push_clean(&mut self.tree, &layer.name);
        }
        let n = layers.len();
        let done = match edit {
            Some(LayerEdit::Select(i)) if i < n => {
                *selected = i;
                true
            }
            Some(LayerEdit::Visible(i, visible)) if i < n => {
                layers[i].visible = visible;
                true
            }
            Some(LayerEdit::Amount(i, amount)) if i < n && layers[i].amount.is_some() => {
                layers[i].amount = Some(amount);
                true
            }
            Some(LayerEdit::Move(i, to)) if i < n && to < n => {
                if i < to {
                    layers[i..=to].rotate_left(1);
                } else {
                    layers[to..=i].rotate_right(1);
                }
                *selected = to;
                true
            }
            _ => false,
        };
        Response { clicked: false, changed: done }
    }
}

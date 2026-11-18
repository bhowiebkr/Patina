use calc::{KEY, ORANGE, case, key, lcd};
use patina_sdk::{Pointer, Ui, node};

/// The size of the case, and the number of digits of the LCD.
const SIZE: [f32; 2] = [168.0, 118.0];
const DIGITS: usize = 8;
/// The keys of the operations, from the left. Each key is 32 x 25 pixels, 38 pixels apart.
const OPS: [char; 4] = ['+', '-', 'x', '/'];
const KEYS_AT: [f32; 2] = [11.0, 80.0];
const KEY_SIZE: [f32; 2] = [32.0, 25.0];
const STEP: f32 = 38.0;

/// Gives a + b, a - b, a x b or a / b. Click a key to select the operation, or set the **op**
/// parameter. A division by 0 shows `Err`.
#[node(free_layout)]
pub struct Operation {
    op: char,
    result: f64,
}

impl Operation {
    pub fn init() -> Self {
        Operation { op: '+', result: 0.0 }
    }

    pub fn process(&mut self, a: f64, b: f64, op: char) -> f64 {
        self.op = op;
        self.result = match op {
            '-' => a - b,
            'x' | '*' => a * b,
            '/' => a / b,
            _ => a + b,
        };
        self.result
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let selected = OPS.iter().position(|&o| o == self.op || (o == 'x' && self.op == '*')).unwrap_or(0);
        let mut clicked = None;
        {
            let mut c = ui.canvas(&format!("Operation {} = {}", OPS[selected], self.result), SIZE);
            for p in c.pointer() {
                if let Pointer::Down([x, y]) = *p {
                    let i = ((x - KEYS_AT[0]) / STEP).floor();
                    let on_key = (0.0..4.0).contains(&i) && x - KEYS_AT[0] - i * STEP < KEY_SIZE[0];
                    if on_key && (KEYS_AT[1]..KEYS_AT[1] + KEY_SIZE[1]).contains(&y) {
                        clicked = Some(OPS[i as usize]);
                    }
                }
            }
            case(&mut c, SIZE, &format!("A {} B =", OPS[selected]));
            lcd(&mut c, [14.0, 28.0], DIGITS, self.result);
            for (i, op) in OPS.iter().enumerate() {
                let at = [KEYS_AT[0] + i as f32 * STEP, KEYS_AT[1]];
                let lit = i == selected;
                key(&mut c, at, KEY_SIZE, &op.to_string(), if lit { ORANGE } else { KEY }, lit);
            }
        }
        if let Some(op) = clicked.filter(|&op| op != OPS[selected]) {
            self.op = op;
            ui.set_param("op", op);
        }
        ui.pin("a", [0.0, 36.0]);
        ui.pin("b", [0.0, 56.0]);
        ui.pin("out", [SIZE[0], 46.0]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_selects_the_operation() {
        let mut op = Operation::init();
        assert_eq!(op.process(12.0, 3.0, '\0'), 15.0, "no op text adds");
        assert_eq!(op.process(12.0, 3.0, 'x'), 36.0);
        assert!(op.process(1.0, 0.0, '/').is_infinite());
        // A click on the middle of the `-` key, then a click between two keys.
        let mut ui = Ui::new(0, "0 down 65 92\n0 down 83 92");
        op.ui(&mut ui);
        assert_eq!(op.op, '-');
        assert_eq!(op.process(12.0, 3.0, op.op), 9.0);
        let mut ui = Ui::new(0, "");
        op.ui(&mut ui);
        assert!(ui.tree().starts_with("canvas\tOperation - = 9\t168\t118\t"), "{}", ui.tree());
    }
}

use calc::{KEY, ORANGE, case, fit, key, lcd_text};
use patina_sdk::{Pointer, Ui, node};

/// The size of the case, and the number of digits of the LCD.
const SIZE: [f32; 2] = [168.0, 210.0];
const DIGITS: usize = 8;
/// The top left corner of the keys, the size of one key cell, and the gap between two keys.
const KEYS_AT: [f32; 2] = [11.0, 80.0];
const CELL: [f32; 2] = [38.0, 31.0];
const GAP: f32 = 6.0;
/// The keys: the label, the column, the row, and the width and the height in cells.
const KEYS: [(&str, u8, u8, u8, u8); 14] = [
    ("7", 0, 0, 1, 1),
    ("8", 1, 0, 1, 1),
    ("9", 2, 0, 1, 1),
    ("C", 3, 0, 1, 1),
    ("4", 0, 1, 1, 1),
    ("5", 1, 1, 1, 1),
    ("6", 2, 1, 1, 1),
    ("<", 3, 1, 1, 1),
    ("1", 0, 2, 1, 1),
    ("2", 1, 2, 1, 1),
    ("3", 2, 2, 1, 1),
    ("+/-", 3, 2, 1, 2),
    ("0", 0, 3, 2, 1),
    (".", 2, 3, 1, 1),
];

/// Gives the number that the user types on the keys. **C** clears the number, **<** removes the
/// last digit, and **+/-** changes the sign. The **value** parameter keeps the number.
#[node(free_layout)]
pub struct Keypad {
    /// The text on the LCD, for example `12.` while the user types a decimal number.
    entry: String,
}

impl Keypad {
    pub fn init() -> Self {
        Keypad { entry: "0".to_owned() }
    }

    pub fn process(&mut self, value: f64) -> f64 {
        // A change in the Inspector or an undo sets the value, so the LCD shows it.
        if self.entry.parse::<f64>().ok() != Some(value) {
            self.entry = fit(value, DIGITS);
        }
        value
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let before = self.entry.clone();
        {
            let mut c = ui.canvas(&format!("Keypad {}", self.entry), SIZE);
            for p in c.pointer() {
                if let Pointer::Down(at) = p
                    && let Some(&(label, ..)) = KEYS.iter().find(|k| inside(rect(**k), *at))
                {
                    self.press(label);
                }
            }
            case(&mut c, SIZE, "KEYPAD");
            lcd_text(&mut c, [14.0, 28.0], DIGITS, &self.entry);
            for k in KEYS {
                let (at, size) = rect(k);
                key(&mut c, at, size, k.0, if k.0 == "C" { ORANGE } else { KEY }, false);
            }
        }
        if self.entry != before {
            ui.set_param("value", self.entry.parse::<f64>().unwrap_or(0.0));
        }
        ui.pin("out", [SIZE[0], 46.0]);
    }

    /// Applies the key `label` to the entry.
    fn press(&mut self, label: &str) {
        // An exponent or an error does not take more digits, so a key starts a new number.
        if self.entry.contains(['e', 'r']) {
            self.entry = "0".to_owned();
        }
        let digits = self.entry.chars().filter(char::is_ascii_digit).count();
        match label {
            "C" => self.entry = "0".to_owned(),
            "<" => {
                self.entry.pop();
                if self.entry.is_empty() || self.entry == "-" {
                    self.entry = "0".to_owned();
                }
            }
            "+/-" => {
                if let Some(rest) = self.entry.strip_prefix('-') {
                    self.entry = rest.to_owned();
                } else if self.entry != "0" {
                    self.entry.insert(0, '-');
                }
            }
            "." if !self.entry.contains('.') => self.entry.push('.'),
            "." => {}
            digit if self.entry == "0" => self.entry = digit.to_owned(),
            digit if digits < DIGITS => self.entry.push_str(digit),
            _ => {}
        }
    }
}

/// The top left corner and the size of a key.
fn rect((_, col, row, w, h): (&str, u8, u8, u8, u8)) -> ([f32; 2], [f32; 2]) {
    let at = [KEYS_AT[0] + f32::from(col) * CELL[0], KEYS_AT[1] + f32::from(row) * CELL[1]];
    (at, [f32::from(w) * CELL[0] - GAP, f32::from(h) * CELL[1] - GAP])
}

fn inside(([x, y], [w, h]): ([f32; 2], [f32; 2]), [px, py]: [f32; 2]) -> bool {
    (x..x + w).contains(&px) && (y..y + h).contains(&py)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pointer event of a click on the middle of the key `label`.
    fn click(label: &str) -> String {
        let (at, size) = rect(*KEYS.iter().find(|k| k.0 == label).unwrap());
        format!("0 down {} {}\n", at[0] + size[0] / 2.0, at[1] + size[1] / 2.0)
    }

    fn type_keys(keypad: &mut Keypad, labels: &[&str]) -> Ui {
        let mut ui = Ui::new(0, &labels.iter().map(|l| click(l)).collect::<String>());
        keypad.ui(&mut ui);
        ui
    }

    #[test]
    fn the_keys_type_a_number() {
        let mut keypad = Keypad::init();
        type_keys(&mut keypad, &["1", "2", ".", ".", "5", "+/-"]);
        assert_eq!(keypad.entry, "-12.5");
        type_keys(&mut keypad, &["<", "<", "<", "<", "0", "7"]);
        assert_eq!(keypad.entry, "7");
        type_keys(&mut keypad, &["9"; 9]);
        assert_eq!(keypad.entry, "79999999", "the LCD has 8 digits");
        type_keys(&mut keypad, &["C", "+/-"]);
        assert_eq!(keypad.entry, "0");

        // The Inspector sets the value, and the next key starts a new number after an exponent.
        assert_eq!(keypad.process(1e21), 1e21);
        assert_eq!(keypad.entry, "1e21");
        type_keys(&mut keypad, &["3"]);
        assert_eq!(keypad.entry, "3");
        // The editor calls `ui` again without events, so the canvas text shows the new entry.
        let ui = type_keys(&mut keypad, &[]);
        assert!(ui.tree().starts_with("canvas\tKeypad 3\t168\t210\t"), "{}", ui.tree());
    }
}

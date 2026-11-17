use patina_sdk::{Ui, node, project_path};

const WORDS: usize = 16;
/// The operations of the CPU, in the order of their 3-bit codes.
const OPS: [&str; 8] = ["LDI", "ADD", "OUT", "JMP", "JC", "ST", "LD", "JZ"];
const TEXT: [u8; 3] = [238, 238, 235];
const OFF: [u8; 3] = [102, 102, 97];
const ON: [u8; 3] = [223, 166, 75];
const BACK: [u8; 3] = [24, 24, 23];
const MARK: [u8; 3] = [70, 54, 26];
const ROW: f32 = 16.0;

/// The program memory of the CPU, with an assembler. It gives the instruction word at address
/// `pc`: bits 0 to 3 are the operand and bits 4 to 6 the operation. `file` is the assembly source
/// in the project folder. The GUI edits the source and writes the file again after each change.
#[node(color = "#655219")]
pub struct Program {
    file: String,
    code: String,
    words: [u8; WORDS],
    len: usize,
    errors: Vec<String>,
    pc: u8,
}

impl Program {
    pub fn init() -> Self {
        Program { file: String::new(), code: String::new(), words: [0; WORDS], len: 0, errors: Vec::new(), pc: 0 }
    }

    pub fn process(&mut self, pc: &u8, file: &str) -> u8 {
        if self.file != file {
            file.clone_into(&mut self.file);
            match std::fs::read_to_string(project_path(file)) {
                Ok(code) => {
                    self.code = code;
                    self.assemble();
                }
                Err(e) if !file.is_empty() => self.errors = vec![format!("Cannot read {file}: {e}")],
                Err(_) => {}
            }
        }
        self.pc = pc % WORDS as u8;
        self.words[usize::from(self.pc)]
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        if ui.code_field("Program", &mut self.code).changed() {
            self.assemble();
            // The source is written also when it has errors, so no text is lost.
            if !self.file.is_empty()
                && let Err(e) = std::fs::write(project_path(&self.file), &self.code)
            {
                self.errors.push(format!("Cannot write {}: {e}", self.file));
            }
        }
        if self.file.is_empty() {
            ui.label("Set the file parameter to keep the program.");
        }
        if !self.errors.is_empty() {
            ui.label("The CPU runs the last good program.");
        }
        for error in &self.errors {
            ui.label(error);
        }
        let size = [180.0, ROW * WORDS as f32 + 4.0];
        let mut canvas = ui.canvas("Memory", size);
        canvas.rect([0.0, 0.0], size, BACK, true);
        for (address, &word) in self.words.iter().enumerate() {
            let y = 2.0 + address as f32 * ROW;
            let here = address == usize::from(self.pc);
            if here {
                canvas.rect([0.0, y], [size[0], ROW], MARK, true);
            }
            let color = if here { ON } else if address < self.len { TEXT } else { OFF };
            canvas.text([6.0, y + 12.0], color, &format!("{address:X}  {word:07b}  {}", disassemble(word)));
        }
    }

    /// Assembles the code. With errors, the words of the last good program stay.
    fn assemble(&mut self) {
        match assemble(&self.code) {
            Ok((words, len)) => {
                (self.words, self.len) = (words, len);
                self.errors.clear();
            }
            Err(errors) => self.errors = errors,
        }
    }
}

/// Assembles a program of up to 16 instructions. A line has an optional label `name:`, an
/// optional instruction and an optional comment after `;`. An operand is a number from 0 to 15
/// (decimal, `0x` or `0b`) or a label. `OUT` has no operand. Names are not case-sensitive.
fn assemble(code: &str) -> Result<([u8; WORDS], usize), Vec<String>> {
    let (mut labels, mut lines, mut errors) = (Vec::new(), Vec::new(), Vec::new());
    for (n, line) in code.lines().enumerate().map(|(i, l)| (i + 1, l)) {
        let mut line = line.split(';').next().unwrap_or("").trim();
        if let Some((label, rest)) = line.split_once(':') {
            let label = label.trim();
            if !is_name(label) {
                errors.push(format!("Line {n}: \"{label}\" is not a label name."));
            } else if labels.iter().any(|(l, _): &(&str, usize)| l.eq_ignore_ascii_case(label)) {
                errors.push(format!("Line {n}: the label \"{label}\" is already used."));
            } else {
                labels.push((label, lines.len()));
            }
            line = rest.trim();
        }
        let mut parts = line.split_whitespace();
        if let Some(op) = parts.next() {
            lines.push((n, op, parts.next(), parts.next().is_some()));
        }
    }
    if lines.len() > WORDS {
        errors.push(format!("The program has {} instructions. The memory has {WORDS} words.", lines.len()));
    }
    let mut words = [0; WORDS];
    for (word, &(n, op, operand, extra)) in words.iter_mut().zip(&lines) {
        let Some(code) = OPS.iter().position(|o| o.eq_ignore_ascii_case(op)) else {
            errors.push(format!("Line {n}: \"{op}\" is not an instruction."));
            continue;
        };
        let k = match (OPS[code], operand) {
            _ if extra => Err("too many operands.".to_owned()),
            ("OUT", None) => Ok(0),
            ("OUT", Some(_)) => Err("OUT has no operand.".to_owned()),
            (op, None) => Err(format!("{op} needs an operand.")),
            (_, Some(operand)) => value(operand, &labels),
        };
        match k {
            Ok(k) => *word = (code as u8) << 4 | k,
            Err(e) => errors.push(format!("Line {n}: {e}")),
        }
    }
    if errors.is_empty() { Ok((words, lines.len())) } else { Err(errors) }
}

/// The value of an operand: a number from 0 to 15 or the address of a label.
fn value(operand: &str, labels: &[(&str, usize)]) -> Result<u8, String> {
    let lower = operand.to_ascii_lowercase();
    let number = if let Some(hex) = lower.strip_prefix("0x") {
        u8::from_str_radix(hex, 16).ok()
    } else if let Some(bin) = lower.strip_prefix("0b") {
        u8::from_str_radix(bin, 2).ok()
    } else if lower.starts_with(|c: char| c.is_ascii_digit()) {
        lower.parse().ok()
    } else {
        let label = labels.iter().find(|(l, _)| l.eq_ignore_ascii_case(operand));
        return label.map(|&(_, address)| address as u8).ok_or_else(|| format!("the label \"{operand}\" is not defined."));
    };
    number.filter(|&n| usize::from(n) < WORDS).ok_or_else(|| format!("the operand \"{operand}\" must be a number from 0 to 15."))
}

fn is_name(text: &str) -> bool {
    text.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_') && text.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// The assembly text of a word.
fn disassemble(word: u8) -> String {
    let op = OPS[usize::from(word >> 4 & 7)];
    if op == "OUT" { op.to_owned() } else { format!("{op} {}", word & 15) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assembles_the_fibonacci_program() {
        let (words, len) = assemble(include_str!("../../../programs/fibonacci.asm")).unwrap();
        let program = [(0, 0), (5, 2), (0, 1), (5, 1), (2, 0), (1, 2), (4, 13), (5, 3), (6, 1), (5, 2), (6, 3), (5, 1), (3, 4), (0, 0), (2, 0), (7, 0)];
        assert_eq!(words, program.map(|(op, k)| op << 4 | k));
        assert_eq!(len, 16);
        assert_eq!(disassemble(words[6]), "JC 13");
        assert_eq!(disassemble(words[4]), "OUT");
    }

    #[test]
    fn reads_numbers_labels_and_comments() {
        let (words, len) = assemble("start: ldi 0x0F ; a comment\n\nadd 0b11\nend:\n jz start\nJMP End").unwrap();
        assert_eq!(&words[..4], &[0x0F, 0x13, 0x70, 0x32]);
        assert_eq!(len, 4);
    }

    #[test]
    fn reports_each_error_with_its_line() {
        let errors = assemble("LDI 16\nFOO 1\nOUT 1\nJMP\nJMP nowhere\nADD 1 2\n2x: LDI 1\na: OUT\nA: OUT").unwrap_err();
        assert_eq!(
            errors,
            [
                "Line 7: \"2x\" is not a label name.",
                "Line 9: the label \"A\" is already used.",
                "Line 1: the operand \"16\" must be a number from 0 to 15.",
                "Line 2: \"FOO\" is not an instruction.",
                "Line 3: OUT has no operand.",
                "Line 4: JMP needs an operand.",
                "Line 5: the label \"nowhere\" is not defined.",
                "Line 6: too many operands.",
            ]
        );
        assert!(assemble(&"OUT\n".repeat(17)).unwrap_err()[0].starts_with("The program has 17 instructions."));
    }

    #[test]
    fn keeps_the_last_good_program_when_the_code_has_errors() {
        let mut program = Program::init();
        program.code = "LDI 3\nOUT".to_owned();
        program.assemble();
        program.code = "LDI 99".to_owned();
        program.assemble();
        assert_eq!(program.process(&17, ""), 0x20);
        assert_eq!((program.len, program.errors.len()), (2, 1));
    }
}

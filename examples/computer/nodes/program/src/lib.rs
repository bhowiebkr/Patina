use std::io::Write as _;
use std::path::PathBuf;

use patina_sdk::{Mode, Ui, node, project_path};

const WORDS: usize = 16;
/// The operations of the CPU, in the order of their 3-bit codes.
const OPS: [&str; 8] = ["LDI", "ADD", "OUT", "JMP", "JC", "ST", "LD", "JZ"];
const TEXT: [u8; 3] = [238, 238, 235];
const OFF: [u8; 3] = [102, 102, 97];
const ON: [u8; 3] = [223, 166, 75];
const BACK: [u8; 3] = [24, 24, 23];
const MARK: [u8; 3] = [70, 54, 26];
const ROW: f32 = 16.0;
/// The source of a program that **New** makes.
const NEW: &str = "; A new program. Each line has an optional label, an instruction and a comment.
; LDI k: A = k. ADD a: A = A + RAM a. OUT: write A. JMP a, JC a, JZ a: jump.
; ST a: RAM a = A. LD a: A = RAM a.
        LDI 1
        ST 0        ; RAM 0 = 1
        LDI 0
loop:   OUT
        ADD 0       ; A = A + 1
        JMP loop
";

/// An assembler error: the source line, from 0, if the error has one, and the text.
type Error = (Option<usize>, String);

/// An assembled program: the words, the source line of each word, from 0, and the count of words.
#[derive(Clone, Copy, Debug, Default)]
struct Image {
    words: [u8; WORDS],
    lines: [usize; WORDS],
    len: usize,
}

/// The program memory of the CPU, with an assembler. It gives the instruction word at address
/// `pc`: bits 0 to 3 are the operand and bits 4 to 6 the operation. `file` is the assembly source
/// in the project folder. The GUI selects the file, makes a new file and edits the source, and
/// writes the file again after each change. The full size shows the line numbers, the line that
/// runs and the errors.
#[node(color = "#655219", full_gui)]
pub struct Program {
    file: String,
    /// The value of the file picker. A change writes the `file` parameter (4.3).
    pick: String,
    code: String,
    image: Image,
    errors: Vec<Error>,
    pc: u8,
}

impl Program {
    pub fn init() -> Self {
        Program { file: String::new(), pick: String::new(), code: String::new(), image: Image::default(), errors: Vec::new(), pc: 0 }
    }

    pub fn process(&mut self, pc: &u8, file: &str) -> u8 {
        if self.file != file {
            file.clone_into(&mut self.file);
            match std::fs::read_to_string(project_path(file)) {
                Ok(code) => {
                    self.code = code;
                    self.assemble();
                }
                Err(e) if !file.is_empty() => self.errors = vec![(None, format!("Cannot read {file}: {e}"))],
                Err(_) => {}
            }
        }
        self.pc = pc % WORDS as u8;
        self.image.words[usize::from(self.pc)]
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let mode = ui.mode();
        self.pick.clone_from(&self.file);
        if ui.asset_picker("File", &mut self.pick, &["asm"]).changed() && self.pick != self.file {
            ui.set_param("file", &self.pick);
        }
        if mode == Mode::Compact {
            ui.label(&self.status());
            return;
        }
        if ui.button("New").clicked() {
            match new_file(project_path) {
                Ok(path) => ui.set_param("file", path),
                Err(e) => self.errors.push((None, format!("Cannot write a new program: {e}"))),
            }
        }
        let changed = if mode == Mode::Full {
            let pc = usize::from(self.pc);
            let current = (self.errors.is_empty() && pc < self.image.len).then(|| self.image.lines[pc]);
            let marks = self.errors.iter().filter_map(|(line, text)| line.map(|l| (l, text.as_str())));
            ui.code_editor("Program", &mut self.code, current, marks).changed()
        } else {
            ui.code_field("Program", &mut self.code).changed()
        };
        if changed {
            self.assemble();
            // The source is written also when it has errors, so no text is lost.
            if !self.file.is_empty()
                && let Err(e) = std::fs::write(project_path(&self.file), &self.code)
            {
                self.errors.push((None, format!("Cannot write {}: {e}", self.file)));
            }
        }
        if self.file.is_empty() {
            ui.label("Select a file or click New to keep the program.");
        }
        if !self.errors.is_empty() {
            ui.label("The CPU runs the last good program.");
        }
        for (_, error) in &self.errors {
            ui.label(error);
        }
        let size = [180.0, ROW * WORDS as f32 + 4.0];
        let mut canvas = ui.canvas("Memory", size);
        canvas.rect([0.0, 0.0], size, BACK, true);
        for (address, &word) in self.image.words.iter().enumerate() {
            let y = 2.0 + address as f32 * ROW;
            let here = address == usize::from(self.pc);
            if here {
                canvas.rect([0.0, y], [size[0], ROW], MARK, true);
            }
            let color = if here { ON } else if address < self.image.len { TEXT } else { OFF };
            canvas.text([6.0, y + 12.0], color, &format!("{address:X}  {word:07b}  {}", disassemble(word)));
        }
    }

    /// The status line of the node body: the first error, or the address and the instruction that runs.
    fn status(&self) -> String {
        match self.errors.first() {
            Some((_, error)) => error.clone(),
            None if self.file.is_empty() => "No program file.".to_owned(),
            None => format!("PC {:X}: {}", self.pc, disassemble(self.image.words[usize::from(self.pc)])),
        }
    }

    /// Assembles the code. With errors, the words of the last good program stay.
    fn assemble(&mut self) {
        match assemble(&self.code) {
            Ok(image) => {
                self.image = image;
                self.errors.clear();
            }
            Err(errors) => self.errors = errors,
        }
    }
}

/// Writes a new program to the first free `programs/newN.asm` and gives its path. `path` gives the
/// full path of a path in the project.
fn new_file(path: impl Fn(&str) -> PathBuf) -> std::io::Result<String> {
    std::fs::create_dir_all(path("programs"))?;
    for n in 1..1000 {
        let name = format!("programs/new{n}.asm");
        match std::fs::OpenOptions::new().write(true).create_new(true).open(path(&name)) {
            Ok(mut file) => return file.write_all(NEW.as_bytes()).map(|()| name),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
    }
    Err(std::io::Error::other("programs/new1.asm to programs/new999.asm are used"))
}

/// Assembles a program of up to 16 instructions. A line has an optional label `name:`, an
/// optional instruction and an optional comment after `;`. An operand is a number from 0 to 15
/// (decimal, `0x` or `0b`) or a label. `OUT` has no operand. Names are not case-sensitive.
fn assemble(code: &str) -> Result<Image, Vec<Error>> {
    let (mut labels, mut lines, mut errors) = (Vec::new(), Vec::new(), Vec::new());
    let error = |n: usize, text: String| (Some(n), format!("Line {}: {text}", n + 1));
    for (n, line) in code.lines().enumerate() {
        let mut line = line.split(';').next().unwrap_or("").trim();
        if let Some((label, rest)) = line.split_once(':') {
            let label = label.trim();
            if !is_name(label) {
                errors.push(error(n, format!("\"{label}\" is not a label name.")));
            } else if labels.iter().any(|(l, _): &(&str, usize)| l.eq_ignore_ascii_case(label)) {
                errors.push(error(n, format!("the label \"{label}\" is already used.")));
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
    if let Some(&(n, ..)) = lines.get(WORDS) {
        errors.push((Some(n), format!("The program has {} instructions. The memory has {WORDS} words.", lines.len())));
    }
    let mut image = Image { len: lines.len().min(WORDS), ..Image::default() };
    for ((word, at), &(n, op, operand, extra)) in image.words.iter_mut().zip(&mut image.lines).zip(&lines) {
        *at = n;
        let Some(code) = OPS.iter().position(|o| o.eq_ignore_ascii_case(op)) else {
            errors.push(error(n, format!("\"{op}\" is not an instruction.")));
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
            Err(e) => errors.push(error(n, e)),
        }
    }
    if errors.is_empty() { Ok(image) } else { Err(errors) }
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
        let image = assemble(include_str!("../../../programs/fibonacci.asm")).unwrap();
        let program = [(0, 0), (5, 2), (0, 1), (5, 1), (2, 0), (1, 2), (4, 13), (5, 3), (6, 1), (5, 2), (6, 3), (5, 1), (3, 4), (0, 0), (2, 0), (7, 0)];
        assert_eq!(image.words, program.map(|(op, k)| op << 4 | k));
        assert_eq!(image.len, 16);
        assert_eq!(disassemble(image.words[6]), "JC 13");
        assert_eq!(disassemble(image.words[4]), "OUT");
    }

    #[test]
    fn assembles_each_example_program() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../programs");
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let code = std::fs::read_to_string(&path).unwrap();
            assert!(assemble(&code).is_ok_and(|i| i.len <= 16), "{}", path.display());
        }
    }

    #[test]
    fn reads_numbers_labels_and_comments() {
        let image = assemble("start: ldi 0x0F ; a comment\n\nadd 0b11\nend:\n jz start\nJMP End").unwrap();
        assert_eq!(&image.words[..4], &[0x0F, 0x13, 0x70, 0x32]);
        assert_eq!((image.len, &image.lines[..4]), (4, &[0, 2, 4, 5][..]));
        assert!(assemble(NEW).is_ok());
    }

    #[test]
    fn reports_each_error_with_its_line() {
        let errors = assemble("LDI 16\nFOO 1\nOUT 1\nJMP\nJMP nowhere\nADD 1 2\n2x: LDI 1\na: OUT\nA: OUT").unwrap_err();
        assert_eq!(
            errors.iter().map(|(_, text)| text.as_str()).collect::<Vec<_>>(),
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
        assert_eq!(errors.iter().map(|&(line, _)| line).collect::<Vec<_>>(), [6, 8, 0, 1, 2, 3, 4, 5].map(Some));
        let long = assemble(&"OUT\n".repeat(17)).unwrap_err();
        assert!(long[0].1.starts_with("The program has 17 instructions.") && long[0].0 == Some(16));
    }

    #[test]
    fn keeps_the_last_good_program_when_the_code_has_errors() {
        let mut program = Program::init();
        program.code = "LDI 3\nOUT".to_owned();
        program.assemble();
        program.code = "LDI 99".to_owned();
        program.assemble();
        assert_eq!(program.process(&17, ""), 0x20);
        assert_eq!((program.image.len, program.errors.len()), (2, 1));
        assert_eq!(program.status(), "Line 1: the operand \"99\" must be a number from 0 to 15.");
    }

    #[test]
    fn new_makes_the_first_free_file() {
        let root = std::env::temp_dir().join(format!("patina-program-new-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let path = |p: &str| root.join(p);
        assert_eq!(new_file(path).unwrap(), "programs/new1.asm");
        std::fs::remove_file(root.join("programs/new1.asm")).unwrap();
        std::fs::write(root.join("programs/new2.asm"), "").unwrap();
        assert_eq!(new_file(path).unwrap(), "programs/new1.asm");
        assert_eq!(new_file(path).unwrap(), "programs/new3.asm");
        assert_eq!(std::fs::read_to_string(root.join("programs/new3.asm")).unwrap(), NEW);
        std::fs::remove_dir_all(&root).unwrap();
    }
}

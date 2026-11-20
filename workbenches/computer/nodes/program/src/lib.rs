use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime};

use patina_sdk::{Mode, Syntax, Ui, node, project_path};

const WORDS: usize = 256;
/// The rows of the memory view.
const SHOWN: usize = 16;
/// The operations of the CPU, in the order of their 4-bit codes.
const OPS: [&str; 16] = ["LDI", "ADDI", "SUBI", "ANDI", "LD", "ADD", "SUB", "AND", "ST", "STX", "LDX", "TAX", "OUT", "JMP", "JC", "JZ"];
/// The colors of the code field: the comments and the instructions (4.3).
const ASM: Syntax = Syntax { comment: ";", words: &OPS };
/// The operations without an operand.
const BARE: [&str; 2] = ["TAX", "OUT"];
const TEXT: [u8; 3] = [238, 238, 235];
const OFF: [u8; 3] = [102, 102, 97];
const ON: [u8; 3] = [223, 166, 75];
const BACK: [u8; 3] = [24, 24, 23];
const MARK: [u8; 3] = [70, 54, 26];
const ROW: f32 = 16.0;
/// The time between two checks of the modified time of the file.
const RECHECK: std::time::Duration = std::time::Duration::from_millis(500);
/// The source of a program that **New** makes.
const NEW: &str = "; A new program. Each line has an optional label, an instruction and a comment.
; A line NAME = number gives a name to the number.
; LDI k: A = k. ADDI k, SUBI k, ANDI k: A = A + k, A - k, A and k.
; LD a: A = M[a]. ADD a, SUB a, AND a: the same with M[a]. ST a: M[a] = A.
; LDX a, STX a: LD and ST at the address a + X. TAX: X = A. OUT: write A.
; JMP a, JC a, JZ a: jump, jump at a carry, jump when A is 0.
STEP = 0x10         ; an address in the RAM
        LDI 1
        ST STEP     ; M[STEP] = 1
        LDI 0
loop:   OUT
        ADD STEP    ; A = A + M[STEP]
        JMP loop
";

/// An assembler error: the source line, from 0, if the error has one, and the text.
type Error = (Option<usize>, String);

/// An assembled program: the words, the source line of each word, from 0, and the count of words.
/// The high byte of a word is the operation and the low byte the operand.
#[derive(Clone, Copy, Debug)]
struct Image {
    words: [u16; WORDS],
    lines: [usize; WORDS],
    len: usize,
}

impl Default for Image {
    fn default() -> Self {
        Image { words: [0; WORDS], lines: [0; WORDS], len: 0 }
    }
}

/// The program memory of the CPU, with an assembler. It gives the instruction at address `pc`:
/// the operation on `out0`, 0 to 15, and the operand on `out1`. `file` is the assembly source
/// in the project folder. The GUI selects the file, makes a new file and edits the source, and
/// writes the file again after each change. A change of the file from outside Patina shows in
/// half a second. The full size shows the line numbers, the line that runs and the errors.
#[node(color = "#655219", full_gui)]
pub struct Program {
    file: String,
    /// The value of the file picker. A change writes the `file` parameter (4.3).
    pick: String,
    code: String,
    image: Image,
    errors: Vec<Error>,
    pc: u8,
    /// The modified time of the file at the last read or write.
    stamp: Option<SystemTime>,
    checked: Instant,
}

impl Program {
    pub fn init() -> Self {
        Program { file: String::new(), pick: String::new(), code: String::new(), image: Image::default(), errors: Vec::new(), pc: 0, stamp: None, checked: Instant::now() }
    }

    pub fn process(&mut self, pc: &u8, file: &str) -> (u8, u8) {
        if self.file != file {
            file.clone_into(&mut self.file);
            self.read(&project_path(file));
        } else if !file.is_empty() && self.checked.elapsed() >= RECHECK {
            self.checked = Instant::now();
            self.read_changed(&project_path(file));
        }
        self.pc = *pc;
        let [op, k] = self.image.words[usize::from(self.pc)].to_be_bytes();
        (op, k)
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
            ui.code_editor("Program", &mut self.code, ASM, current, marks).changed()
        } else {
            ui.code_field("Program", &mut self.code, ASM).changed()
        };
        if changed {
            self.assemble();
            // The source is written also when it has errors, so no text is lost.
            if !self.file.is_empty() {
                let path = project_path(&self.file);
                match std::fs::write(&path, &self.code) {
                    // The node does not read its own write again.
                    Ok(()) => self.stamp = modified(&path),
                    Err(e) => self.errors.push((None, format!("Cannot write {}: {e}", self.file))),
                }
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
        // The view shows 16 words around the PC.
        let pc = usize::from(self.pc);
        let first = pc.saturating_sub(SHOWN / 2).min(WORDS - SHOWN);
        let size = [180.0, ROW * SHOWN as f32 + 4.0];
        let mut canvas = ui.canvas("Memory", size);
        canvas.rect([0.0, 0.0], size, BACK, true);
        for (address, &word) in self.image.words.iter().enumerate().skip(first).take(SHOWN) {
            let y = 2.0 + (address - first) as f32 * ROW;
            if address == pc {
                canvas.rect([0.0, y], [size[0], ROW], MARK, true);
            }
            let color = if address == pc { ON } else if address < self.image.len { TEXT } else { OFF };
            canvas.text([6.0, y + 12.0], color, format_args!("{address:02X}  {word:03X}  {}", disassemble(word)));
        }
    }

    /// The status line of the node body: the first error, or the address and the instruction that runs.
    fn status(&self) -> String {
        match self.errors.first() {
            Some((_, error)) => error.clone(),
            None if self.file.is_empty() => "No program file.".to_owned(),
            None => format!("PC {:02X}: {}", self.pc, disassemble(self.image.words[usize::from(self.pc)])),
        }
    }

    /// Reads and assembles the file. When the file has an error, the last good program stays.
    fn read(&mut self, path: &Path) {
        self.stamp = modified(path);
        match std::fs::read_to_string(path) {
            Ok(code) => {
                self.code = code;
                self.assemble();
            }
            Err(e) if !self.file.is_empty() => self.errors = vec![(None, format!("Cannot read {}: {e}", self.file))],
            Err(_) => {}
        }
    }

    /// Reads the file again when its modified time is not the time of the last read or write.
    fn read_changed(&mut self, path: &Path) {
        if modified(path) != self.stamp {
            self.read(path);
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

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
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

/// Assembles a program of up to 256 instructions. A line has an optional label `name:`, an
/// optional instruction and an optional comment after `;`. A line `name = number` gives a name to
/// a number. An operand is a number from -128 to 255 (decimal, `0x` or `0b`), a label or a name.
/// `TAX` and `OUT` have no operand. Names are not case-sensitive.
fn assemble(code: &str) -> Result<Image, Vec<Error>> {
    let (mut names, mut lines, mut errors) = (Vec::new(), Vec::new(), Vec::new());
    let error = |n: usize, text: String| (Some(n), format!("Line {}: {text}", n + 1));
    for (n, line) in code.lines().enumerate() {
        let mut line = line.split(';').next().unwrap_or("").trim();
        if let Some((name, text)) = line.split_once('=') {
            let text = text.trim();
            let defined = number(text).ok_or_else(|| format!("\"{text}\" is not a number from -128 to 255.")).and_then(|v| define(&mut names, name.trim(), v));
            if let Err(e) = defined {
                errors.push(error(n, e));
            }
            continue;
        }
        if let Some((label, rest)) = line.split_once(':') {
            // A label after word 255 has no address, and the count error below reports it.
            if let Err(e) = define(&mut names, label.trim(), lines.len() as u8) {
                errors.push(error(n, e));
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
        let op = OPS[code];
        let k = match (operand, BARE.contains(&op)) {
            _ if extra => Err("too many operands.".to_owned()),
            (None, true) => Ok(0),
            (Some(_), true) => Err(format!("{op} has no operand.")),
            (None, false) => Err(format!("{op} needs an operand.")),
            (Some(operand), false) => value(operand, &names),
        };
        match k {
            Ok(k) => *word = (code as u16) << 8 | u16::from(k),
            Err(e) => errors.push(error(n, e)),
        }
    }
    if errors.is_empty() { Ok(image) } else { Err(errors) }
}

/// Adds a label or a name with its value, or gives the error.
fn define<'a>(names: &mut Vec<(&'a str, u8)>, name: &'a str, value: u8) -> Result<(), String> {
    let valid = name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_') && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if !valid {
        Err(format!("\"{name}\" is not a name."))
    } else if names.iter().any(|(n, _)| n.eq_ignore_ascii_case(name)) {
        Err(format!("the name \"{name}\" is already used."))
    } else {
        names.push((name, value));
        Ok(())
    }
}

/// The value of an operand: a number, a label or a name.
fn value(operand: &str, names: &[(&str, u8)]) -> Result<u8, String> {
    if operand.starts_with(|c: char| c.is_ascii_digit() || c == '-') {
        return number(operand).ok_or_else(|| format!("the operand \"{operand}\" must be a number from -128 to 255."));
    }
    let name = names.iter().find(|(n, _)| n.eq_ignore_ascii_case(operand));
    name.map(|&(_, value)| value).ok_or_else(|| format!("the name \"{operand}\" is not defined."))
}

/// A number from -128 to 255, decimal, `0x` or `0b`. A negative number gives its byte in two's
/// complement, so `-1` is 255.
fn number(text: &str) -> Option<u8> {
    let (minus, digits) = text.strip_prefix('-').map_or((false, text), |d| (true, d));
    let lower = digits.to_ascii_lowercase();
    let n = if let Some(hex) = lower.strip_prefix("0x") {
        u16::from_str_radix(hex, 16)
    } else if let Some(bin) = lower.strip_prefix("0b") {
        u16::from_str_radix(bin, 2)
    } else {
        lower.parse()
    };
    match (minus, n.ok()?) {
        (false, n @ 0..=255) => Some(n as u8),
        (true, n @ 0..=128) => Some((n as u8).wrapping_neg()),
        _ => None,
    }
}

/// The assembly text of a word.
fn disassemble(word: u16) -> String {
    let [op, k] = word.to_be_bytes();
    let op = OPS[usize::from(op & 15)];
    if BARE.contains(&op) { op.to_owned() } else { format!("{op} {k}") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assembles_the_fibonacci_program() {
        let image = assemble(include_str!("../../../programs/fibonacci.asm")).unwrap();
        let program = [(0, 0), (8, 0), (0, 1), (12, 0), (8, 1), (5, 0), (14, 0), (8, 2), (4, 1), (8, 0), (4, 2), (13, 3)];
        assert_eq!(image.words[..12], program.map(|(op, k)| op << 8 | k));
        assert_eq!(image.len, 12);
        assert_eq!(disassemble(image.words[6]), "JC 0");
        assert_eq!(disassemble(image.words[3]), "OUT");
    }

    #[test]
    fn assembles_each_example_program() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../programs");
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let code = std::fs::read_to_string(&path).unwrap();
            assert!(assemble(&code).is_ok(), "{}", path.display());
        }
    }

    #[test]
    fn reads_numbers_labels_and_comments() {
        let image = assemble("start: ldi 0x0F ; a comment\n\nadd 0b11\nTOP = -2\nend:\n jz start\nJMP End\nsubi top\ntax").unwrap();
        assert_eq!(&image.words[..6], &[0x000F, 0x0503, 0x0F00, 0x0D02, 0x02FE, 0x0B00]);
        assert_eq!((image.len, &image.lines[..6]), (6, &[0, 2, 5, 6, 7, 8][..]));
        assert_eq!(disassemble(0x0B00), "TAX");
        assert_eq!([number("-128"), number("255"), number("0x"), number("-0b1")], [Some(128), Some(255), None, Some(255)]);
        assert!(assemble(NEW).is_ok());
    }

    #[test]
    fn reports_each_error_with_its_line() {
        let errors = assemble("LDI 256\nFOO 1\nOUT 1\nJMP\nJMP nowhere\nADD 1 2\n2x: LDI 1\na: OUT\nA: OUT\nB = 300\nLDI -129").unwrap_err();
        assert_eq!(
            errors.iter().map(|(_, text)| text.as_str()).collect::<Vec<_>>(),
            [
                "Line 7: \"2x\" is not a name.",
                "Line 9: the name \"A\" is already used.",
                "Line 10: \"300\" is not a number from -128 to 255.",
                "Line 1: the operand \"256\" must be a number from -128 to 255.",
                "Line 2: \"FOO\" is not an instruction.",
                "Line 3: OUT has no operand.",
                "Line 4: JMP needs an operand.",
                "Line 5: the name \"nowhere\" is not defined.",
                "Line 6: too many operands.",
                "Line 11: the operand \"-129\" must be a number from -128 to 255.",
            ]
        );
        assert_eq!(errors.iter().map(|&(line, _)| line).collect::<Vec<_>>(), [6, 8, 9, 0, 1, 2, 3, 4, 5, 10].map(Some));
        let long = assemble(&"OUT\n".repeat(257)).unwrap_err();
        assert!(long[0].1.starts_with("The program has 257 instructions.") && long[0].0 == Some(256));
    }

    #[test]
    fn keeps_the_last_good_program_when_the_code_has_errors() {
        let mut program = Program::init();
        program.code = "LDI 3\nOUT".to_owned();
        program.assemble();
        program.code = "LDI 999".to_owned();
        program.assemble();
        assert_eq!(program.process(&1, ""), (12, 0));
        assert_eq!((program.image.len, program.errors.len()), (2, 1));
        assert_eq!(program.status(), "Line 1: the operand \"999\" must be a number from -128 to 255.");
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

    #[test]
    fn reads_the_file_again_only_after_a_change_of_its_modified_time() {
        let path = std::env::temp_dir().join(format!("patina-program-read-{}.asm", std::process::id()));
        let write = |code: &str, time: SystemTime| {
            std::fs::write(&path, code).unwrap();
            std::fs::File::options().write(true).open(&path).unwrap().set_modified(time).unwrap();
        };
        std::fs::write(&path, "LDI 1").unwrap();
        let mut program = Program::init();
        program.file = "x.asm".to_owned();
        program.read(&path);
        let later = program.stamp.unwrap() + std::time::Duration::from_secs(1);
        write("LDI 2", later);
        program.read_changed(&path);
        assert_eq!(program.image.words[0], 2);
        write("LDI 3", later);
        program.read_changed(&path);
        assert_eq!(program.image.words[0], 2, "the same modified time");
        std::fs::remove_file(&path).unwrap();
        program.read_changed(&path);
        assert!(program.errors[0].1.starts_with("Cannot read x.asm") && program.image.words[0] == 2, "the last good program stays");
    }
}

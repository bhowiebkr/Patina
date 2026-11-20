//! The code field of a node GUI (spec 4.3): a text of many lines, for example a program.

use std::fmt::Write;

use super::{Response, Ui, changed};

/// The colors of a code field (4.3). The editor colors the comments, the words, the numbers, the
/// labels (a name and `:` at the start of a line) and the other names.
#[derive(Clone, Copy, Debug, Default)]
pub struct Syntax<'a> {
    /// The text that starts a comment to the end of the line, for example `;`. Empty for none.
    pub comment: &'a str,
    /// The keywords, for example the instructions of an assembler. The case of a letter does not
    /// matter. The editor removes the spaces of a word.
    pub words: &'a [&'a str],
}

impl Ui {
    /// Shows a field for a text of many lines in a monospace font, for example a program. The
    /// editor sends the text when the field loses the focus, and on Ctrl+Enter. `syntax` gives
    /// the colors of the text.
    pub fn code_field(&mut self, text: &str, value: &mut String, syntax: Syntax) -> Response {
        let event = self.push("code", text).map(unescape);
        self.tree.push('\t');
        escape(&mut self.tree, value);
        self.tree.push('\t');
        for (i, part) in std::iter::once(syntax.comment).chain(syntax.words.iter().copied()).enumerate() {
            if i > 0 {
                self.tree.push(' ');
            }
            self.tree.extend(part.chars().filter(|c| !c.is_whitespace() && !c.is_control()));
        }
        changed(event.map(|v| *value = v))
    }

    /// Shows a code field with line numbers, for the full size. `current` marks a line, from 0,
    /// for example the line that runs. Each item of `errors` marks its line and gives its text.
    pub fn code_editor<'a>(&mut self, text: &str, value: &mut String, syntax: Syntax, current: Option<usize>, errors: impl IntoIterator<Item = (usize, &'a str)>) -> Response {
        let response = self.code_field(text, value, syntax);
        self.tree.push('\t');
        if let Some(line) = current {
            let _ = write!(self.tree, "{line}");
        }
        for (line, error) in errors {
            let _ = write!(self.tree, "\t{line} ");
            escape(&mut self.tree, error);
        }
        response
    }
}

/// A widget line has no line breaks or tabs, so they become `\n` and `\t`, and `\` becomes `\\`.
/// A carriage return is removed, and each other control character becomes a space.
fn escape(tree: &mut String, text: &str) {
    for c in text.chars() {
        match c {
            '\\' => tree.push_str("\\\\"),
            '\n' => tree.push_str("\\n"),
            '\t' => tree.push_str("\\t"),
            '\r' => {}
            c if c.is_control() => tree.push(' '),
            c => tree.push(c),
        }
    }
}

/// Reads the text that `escape` writes. A `\` before another character stays.
fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('\\') => out.push('\\'),
            Some(other) => out.extend(['\\', other]),
            None => out.push('\\'),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_code_field_keeps_line_breaks_tabs_and_backslashes() {
        let text = "LDI 1\n\tOUT ; a\\b\r\nend\\";
        let mut tree = String::new();
        escape(&mut tree, text);
        assert_eq!(tree, "LDI 1\\n\\tOUT ; a\\\\b\\nend\\\\");
        assert_eq!(unescape(&tree), text.replace('\r', ""));
        assert_eq!(unescape("a\\qb\\"), "a\\qb\\");

        let mut value = String::from("old");
        let mut ui = Ui::new(0, "0 JMP 0\\nOUT");
        assert!(ui.code_field("Program", &mut value, Syntax::default()).changed());
        assert_eq!(value, "JMP 0\nOUT");
        assert_eq!(ui.tree(), "code\tProgram\told\t");

        // The syntax goes after the text: the comment mark and the words, with no spaces in a word.
        let mut ui = Ui::new(2, "");
        let asm = Syntax { comment: ";", words: &["JMP", "O UT\t"] };
        ui.code_editor("Program", &mut value, asm, Some(1), [(0, "bad\tword"), (2, "x")]);
        ui.code_editor("Other", &mut value, Syntax::default(), None, std::iter::empty());
        assert_eq!(ui.tree(), "code\tProgram\tJMP 0\\nOUT\t; JMP OUT\t1\t0 bad\\tword\t2 x\ncode\tOther\tJMP 0\\nOUT\t\t");
    }
}

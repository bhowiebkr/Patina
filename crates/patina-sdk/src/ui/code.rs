//! The code field of a node GUI (spec 4.3): a text of many lines, for example a program.

use std::fmt::Write;

use super::{Response, Ui, changed};

impl Ui {
    /// Shows a field for a text of many lines in a monospace font, for example a program. The
    /// editor sends the text when the field loses the focus, and on Ctrl+Enter.
    pub fn code_field(&mut self, text: &str, value: &mut String) -> Response {
        let event = self.push("code", text).map(unescape);
        self.tree.push('\t');
        escape(&mut self.tree, value);
        changed(event.map(|v| *value = v))
    }

    /// Shows a code field with line numbers, for the full size. `current` marks a line, from 0,
    /// for example the line that runs. Each item of `errors` marks its line and gives its text.
    pub fn code_editor<'a>(&mut self, text: &str, value: &mut String, current: Option<usize>, errors: impl IntoIterator<Item = (usize, &'a str)>) -> Response {
        let response = self.code_field(text, value);
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
        assert!(ui.code_field("Program", &mut value).changed());
        assert_eq!(value, "JMP 0\nOUT");
        assert_eq!(ui.tree(), "code\tProgram\told");

        let mut ui = Ui::new(2, "");
        ui.code_editor("Program", &mut value, Some(1), [(0, "bad\tword"), (2, "x")]);
        ui.code_editor("Other", &mut value, None, std::iter::empty());
        assert_eq!(ui.tree(), "code\tProgram\tJMP 0\\nOUT\t1\t0 bad\\tword\t2 x\ncode\tOther\tJMP 0\\nOUT\t");
    }
}

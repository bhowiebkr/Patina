//! The file path widget of a node GUI (spec 4.3).

use super::{Response, Ui, changed, push_clean};

/// What the **Browse** button of `Ui::file_path` selects in the dialog of the platform.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathKind {
    /// A file that exists, for example an image to read.
    Open,
    /// A file to write, for example an export.
    Save,
    /// A folder.
    Folder,
}

impl Ui {
    /// Shows a field for a file path, with a **Browse** button that opens the dialog of the
    /// platform. A path in the project is relative to the project root, with `/`. Give the path to
    /// `patina_sdk::project_path` to get the full path.
    pub fn file_path(&mut self, text: &str, path: &mut String, kind: PathKind) -> Response {
        let event = self.push("path", text).map(str::to_owned);
        self.tree.push_str(match kind {
            PathKind::Open => "\topen\t",
            PathKind::Save => "\tsave\t",
            PathKind::Folder => "\tfolder\t",
        });
        push_clean(&mut self.tree, path);
        changed(event.map(|v| *path = v))
    }
}

use std::fmt::Write;

use super::Ui;

impl Ui {
    /// The space for a view, in logical pixels. None when the host does not supply a size.
    pub fn available_size(&self) -> Option<[f32; 2]> {
        self.available_size
    }

    /// Wraps columns when their minimum width does not fit. Put each column in `Ui::column`.
    pub fn columns(&mut self, min_width: f32, add: impl FnOnce(&mut Ui)) {
        self.push("group", "");
        let width = if min_width.is_finite() {
            min_width.clamp(80.0, 4096.0)
        } else {
            240.0
        };
        let _ = write!(self.tree, "\t{width}");
        add(self);
        self.push("end", "");
    }

    /// A vertical group of controls. Its parent can arrange it with other columns.
    pub fn column(&mut self, add: impl FnOnce(&mut Ui)) {
        self.push("group", "");
        self.tree.push_str("\t0");
        add(self);
        self.push("end", "");
    }
}

pub(super) fn size(events: &str) -> Option<[f32; 2]> {
    let mut words = events
        .lines()
        .find_map(|line| line.strip_prefix("@size "))?
        .split_whitespace();
    let width = words.next()?.parse::<f32>().ok()?;
    let height = words.next()?.parse::<f32>().ok()?;
    (words.next().is_none()
        && width.is_finite()
        && height.is_finite()
        && width > 0.0
        && height > 0.0)
        .then(|| [width.clamp(1.0, 8192.0), height.clamp(1.0, 8192.0)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_metadata_keeps_widget_indices() {
        let mut ui = Ui::new(2, "@size 260 190\n2 1");
        assert_eq!(ui.available_size(), Some([260.0, 190.0]));
        ui.columns(220.0, |ui| {
            ui.column(|ui| assert!(ui.button("Play").clicked()))
        });
        assert_eq!(
            ui.tree(),
            "group\t\t220\ngroup\t\t0\nbutton\tPlay\nend\t\nend\t"
        );
    }

    #[test]
    fn size_rejects_invalid_dimensions() {
        for value in [
            "",
            "@size NaN 30",
            "@size 40 -1",
            "@size 30 40 50",
            "@size inf 30",
        ] {
            assert_eq!(size(value), None);
        }
        assert_eq!(size("@size 99999 40"), Some([8192.0, 40.0]));
    }
}

use crate::{AMBER, INK, MUTED, TEAL, VIOLET, digits, panel, place, plot_frame};
use measurement::{
    ResultData,
    core::{
        results::{Point, Results},
        survey::Layout,
        units::Unit,
    },
};
use patina_sdk::{Pointer, Ui};

pub fn value(point: &Point, channel: usize) -> f64 {
    match channel {
        1 => point.measured_um,
        2 => point.above_min_um,
        3 => point.below_max_um,
        _ => point.flattened_um,
    }
}

pub fn surface(ui: &mut Ui, result: &Results, channel: usize, selected: Option<usize>) -> Option<usize> {
    let mut c = ui.canvas("Surface / select a measured station", [440.0, 250.0]);
    plot_frame(&mut c, [30.0, 20.0, 380.0, 200.0]);
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut xmax, mut ymax) = (1.0_f64, 1.0_f64);
    for p in &result.points {
        lo = lo.min(value(p, channel));
        hi = hi.max(value(p, channel));
        xmax = xmax.max(p.x_mm);
        ymax = ymax.max(p.y_mm);
    }
    let span = (hi - lo).max(0.01);
    let mut picked = None;
    let mut previous = None;
    for p in &result.points {
        let v = value(p, channel);
        let t = ((v - lo) / span).clamp(0.0, 1.0) as f32;
        let color = std::array::from_fn(|i| (TEAL[i] as f32 * (1.0 - t) + AMBER[i] as f32 * t) as u8);
        let x = 30.0 + 380.0 * (p.x_mm / xmax) as f32;
        let y = if result.layout == Layout::Line { 200.0 - 160.0 * t } else { 220.0 - 200.0 * (p.y_mm / ymax) as f32 };
        if result.layout == Layout::Line {
            if let Some(last) = previous {
                c.line(last, [x, y], TEAL);
            }
            previous = Some([x, y]);
            let uncertainty = (160.0 * p.uncertainty_um / span).min(80.0) as f32;
            c.line([x, y - uncertainty], [x, y + uncertainty], MUTED);
        }
        c.circle([x, y], 6.0, color, true);
        if selected == Some(p.station) {
            c.circle([x, y], 10.0, VIOLET, false);
        }
        if result.points.len() <= 36 {
            c.text([(x + 9.0).min(370.0), y - 8.0], INK, format_args!("{v:.2}"));
        }
        for event in c.pointer() {
            if let Pointer::Down(at) = event
                && (at[0] - x).hypot(at[1] - y) < 12.0
            {
                picked = Some(p.station);
            }
        }
    }
    c.text([30.0, 244.0], MUTED, format_args!("{lo:.3} to {hi:.3} um / measured stations only"));
    picked
}

pub struct Display {
    pub channel: usize,
    pub selected: Option<usize>,
    plot: crate::surface::Plot,
    page: usize,
}
impl Default for Display {
    fn default() -> Self {
        Self { channel: 1, selected: None, plot: Default::default(), page: 0 }
    }
}
impl Display {
    pub fn ui(&mut self, ui: &mut Ui, data: &ResultData) {
        panel(ui, [480.0, 822.0], "SURFACE / RESULTS", "MEASURED HEIGHT. ROTATE TO EXPLORE.", VIOLET);
        place(ui, [16.0, 76.0]);
        ui.tabs("Surface channel", &mut self.channel, &["Leveled", "Measured height", "Scrape", "Shim"]);
        self.plot.channel = self.channel;
        self.plot.sync(data);
        place(ui, [16.0, 120.0]);
        let mut c = ui.canvas("Height range / peak to valley", [448.0, 70.0]);
        if let Some(source) = &data.surface {
            let [lo, hi] = source.range(self.channel);
            digits(&mut c, [0., 0.], &format!("{:.3}", hi - lo), TEAL);
            c.text([270., 20.], INK, "um / PEAK TO VALLEY");
            c.text(
                [0., 62.],
                MUTED,
                format_args!(
                    "{} / {} stations measured / {}",
                    source.measured(),
                    source.xy.len(),
                    if self.channel == 1 { "relative to zero" } else { "fitted plane removed" }
                ),
            );
        } else {
            c.text([0., 25.], MUTED, "Measure zero, then start the surface scan.");
        }
        drop(c);
        place(ui, [16., 202.]);
        ui.tabs("Surface display", &mut self.page, &["3D surface", "Stations"]);
        place(ui, [16., 242.]);
        if self.page == 0 {
            self.plot.preview(ui, [448., 350.]);
        } else if let Some(result) = &data.result {
            if let Some(station) = surface(ui, result, self.channel, self.selected) {
                self.selected = Some(station);
            }
        } else {
            ui.label(&data.error);
        }
        let at = (ui.mode() != patina_sdk::Mode::Large).then_some([16., 605.]);
        self.plot.controls(ui, at);
        place(ui, [16., 649.]);
        ui.number_unit("Z display scale", &mut self.plot.scale, 1.0, "x");
        place(ui, [16., 686.]);
        ui.label("Drag to orbit. Arrow keys rotate. Home resets. +/- zooms.");
        place(ui, [16., 716.]);
        if let Some(result) = &data.result {
            ui.label(&format!(
                "Leveled noise {:.3} um / scale uncertainty +/- {:.3} um",
                result.pv_noise_um, result.pv_scale_um
            ));
        }
        place(ui, [16., 748.]);
        if self.channel != 1 && data.surface.as_ref().is_some_and(|s| !s.leveled) {
            ui.label("Plane fit needs more stations. Select Measured height.");
        } else {
            ui.label("Open surface-view for a larger view with pan and wheel zoom.");
        }
    }
}

pub fn table(ui: &mut Ui, data: &ResultData, unit: Unit, selected: &mut Option<usize>, page: &mut u32) -> bool {
    let Some(result) = &data.result else {
        ui.label(&data.error);
        return false;
    };
    let range = page_range(ui, result.points.len(), page);
    let mut row = selected.filter(|i| range.contains(i)).map(|i| i - range.start);
    let mut cells = Vec::with_capacity(range.len() * 10);
    for p in &result.points[range.clone()] {
        cells.extend([
            format!("{}", p.station + 1),
            format!("{:.2}", p.x_mm),
            format!("{:.2}", p.y_mm),
            unit.format(p.measured_um),
            unit.format(p.flattened_um),
            unit.format(p.below_max_um),
            unit.format(p.above_min_um),
            unit.format(p.uncertainty_um),
            p.visits.to_string(),
            p.flags.names().collect::<Vec<_>>().join(", "),
        ]);
    }
    let changed = ui
        .table(
            "Measured stations",
            &[
                "Station",
                "X / mm",
                "Y / mm",
                "Measured",
                "Leveled",
                "Shim",
                "Scrape",
                "Uncertainty",
                "Visits",
                "Checks",
            ],
            &cells,
            &mut row,
        )
        .changed();
    if changed {
        *selected = row.map(|i| i + range.start);
    }
    changed
}

pub fn page_range(ui: &mut Ui, count: usize, page: &mut u32) -> std::ops::Range<usize> {
    let pages = count.div_ceil(500).max(1) as u32;
    if pages > 1 {
        ui.number_unit("Page", page, 1, &format!("of {pages}"));
    }
    *page = (*page).clamp(1, pages);
    let start = (*page as usize - 1) * 500;
    start..(start + 500).min(count)
}

#[cfg(test)]
mod tests {
    #[test]
    fn pages_cover_the_whole_dense_scan() {
        let mut page = 4;
        assert_eq!(super::page_range(&mut patina_sdk::Ui::new(0, ""), 1681, &mut page), 1500..1681);
        page = 99;
        assert_eq!(super::page_range(&mut patina_sdk::Ui::new(0, ""), 1681, &mut page), 1500..1681);
        assert_eq!(page, 4);
    }
    #[test]
    fn channel_values_keep_the_source_meaning() {
        let p = measurement::core::results::Point {
            station: 0,
            label: "A".into(),
            x_mm: 0.0,
            y_mm: 0.0,
            measured_um: 5.0,
            flattened_um: -2.0,
            below_max_um: 7.0,
            above_min_um: 1.0,
            uncertainty_um: 0.1,
            visits: 1,
            t_s: 1.0,
            flags: Default::default(),
        };
        assert_eq!(
            (super::value(&p, 0), super::value(&p, 1), super::value(&p, 2), super::value(&p, 3)),
            (-2.0, 5.0, 1.0, 7.0)
        );
    }
}

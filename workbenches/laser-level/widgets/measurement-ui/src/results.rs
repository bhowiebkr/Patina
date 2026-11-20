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

#[derive(Default)]
pub struct Display {
    pub channel: usize,
    pub selected: Option<usize>,
}
impl Display {
    pub fn ui(&mut self, ui: &mut Ui, data: &ResultData) {
        panel(ui, [472.0, 596.0], "SURFACE / RESULTS", "REFERENCE REMOVED. UNCERTAINTY RETAINED.", VIOLET);
        place(ui, [16.0, 76.0]);
        ui.tabs("Surface channel", &mut self.channel, &["Shape", "Measured", "Scrape", "Shim"]);
        let Some(result) = &data.result else {
            place(ui, [16.0, 125.0]);
            ui.label(&data.error);
            place(ui, [16.0, 164.0]);
            ui.label("Measure zero, then three non-collinear grid stations.");
            place(ui, [16.0, 198.0]);
            ui.label("Return to zero to bracket the laser drift.");
            return;
        };
        place(ui, [16.0, 120.0]);
        let mut c = ui.canvas("Flatness / peak to valley", [440.0, 70.0]);
        digits(&mut c, [0.0, 0.0], &format!("{:.3}", result.pv_um), TEAL);
        c.text([230.0, 20.0], INK, "um / PEAK TO VALLEY");
        c.text(
            [0.0, 62.0],
            MUTED,
            format_args!("Noise estimate {:.3} um / scale +/- {:.3} um", result.pv_noise_um, result.pv_scale_um),
        );
        drop(c);
        place(ui, [16.0, 202.0]);
        if let Some(station) = surface(ui, result, self.channel, self.selected) {
            self.selected = Some(station);
        }
        place(ui, [16.0, 466.0]);
        ui.label(&format!("{} stations / {:?} drift correction", result.points.len(), data.correction));
        place(ui, [16.0, 500.0]);
        if let Some(p) = self.selected.and_then(|id| result.points.iter().find(|p| p.station == id)) {
            ui.label(&format!(
                "Station {} / {:.3} +/- {:.3} um / {} visits",
                p.station + 1,
                value(p, self.channel),
                p.uncertainty_um,
                p.visits
            ));
        } else {
            ui.label("Select a station. Open Survey Data for all values.");
        }
    }
}

pub fn table(ui: &mut Ui, data: &ResultData, unit: Unit, selected: &mut Option<usize>) -> bool {
    let Some(result) = &data.result else {
        ui.label(&data.error);
        return false;
    };
    let mut cells = Vec::with_capacity(result.points.len() * 10);
    for p in &result.points {
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
    ui.table(
        "Measured stations",
        &["Station", "X / mm", "Y / mm", "Measured", "Shape", "Shim", "Scrape", "Uncertainty", "Visits", "Checks"],
        &cells,
        selected,
    )
    .changed()
}

#[cfg(test)]
mod tests {
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

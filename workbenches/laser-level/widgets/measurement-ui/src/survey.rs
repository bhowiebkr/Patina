use crate::{TEAL, panel, place, station_map};
use measurement::{settings, survey::Snapshot};
use patina_sdk::{Mode, Ui};

#[derive(Default)]
pub struct Edits {
    pub request: Option<String>,
    pub options: Option<String>,
}
pub fn controls(ui: &mut Ui, s: &Snapshot) -> Edits {
    let mut edits = Edits::default();
    let mut options = s.options.clone();
    panel(ui, [472.0, 694.0], "SURVEY / MEASUREMENT CONSOLE", "SELECT. SAMPLE. CHECK. KEEP.", TEAL);
    place(ui, [16.0, 77.0]);
    ui.label(&s.phase);
    place(ui, [16.0, 109.0]);
    ui.label(&format!(
        "Station {} / {} visits / {}",
        s.station + 1,
        s.run().map_or(0, |r| r.visits.len()),
        if s.pending { "AWAITING KEEP" } else { "SESSION SAVED AFTER EACH VISIT" }
    ));
    place(ui, [16.0, 145.0]);
    if let Some(run) = s.run()
        && let Some(station) = station_map(ui, run, s.station, [440.0, 190.0])
    {
        options.station = station;
        edits.options = Some(settings::encode(&options));
    }
    place(ui, [16.0, 347.0]);
    ui.progress("Sample / movement progress", s.progress);
    let mut station = options.station as u32 + 1;
    place(ui, [16.0, 385.0]);
    if ui.number("Station", &mut station).changed() {
        options.station = station.saturating_sub(1) as usize;
        edits.options = Some(settings::encode(&options));
    }
    for (i, (label, verb)) in [
        ("Take sample", "sample"),
        ("Keep sample", "keep"),
        ("Discard", "cancel"),
        ("Start scan", "start"),
        ("New run", "new"),
        ("Save session", "save"),
    ]
    .iter()
    .enumerate()
    {
        place(ui, [16.0 + (i % 3) as f32 * 146.0, 427.0 + (i / 3) as f32 * 38.0]);
        if ui.button(label).clicked() {
            edits.request = Some(s.command(verb));
        }
    }
    place(ui, [16.0, 512.0]);
    if ui.number_unit("Sample at least", &mut options.rule.min_s, 0.1, "s").changed() {
        options.rule.max_s = options.rule.max_s.max(options.rule.min_s);
        edits.options = Some(settings::encode(&options));
    }
    place(ui, [16.0, 547.0]);
    if ui.number_unit("Sample at most", &mut options.rule.max_s, 0.1, "s").changed() {
        edits.options = Some(settings::encode(&options));
    }
    let mut target = options.rule.target_um.unwrap_or(0.0);
    place(ui, [16.0, 582.0]);
    if ui.number_unit("Uncertainty target / 0 = off", &mut target, 0.01, "um").changed() {
        options.rule.target_um = (target > 0.0).then_some(target);
        edits.options = Some(settings::encode(&options));
    }
    place(ui, [16.0, 625.0]);
    ui.label(&s.error);
    place(ui, [16.0, 654.0]);
    ui.label(&s.sample.as_ref().map_or_else(
        || "Each sample stays tied to the station where it started.".into(),
        |r| {
            format!(
                "{}/{} frames used / +/- {:.3} px / {}",
                r.used,
                r.frames,
                r.uncertainty_px,
                r.flags.names().collect::<Vec<_>>().join(", ")
            )
        },
    ));
    if ui.mode() != Mode::Large {
        ui.pin("reading", [0.0, 145.0]);
        ui.pin("plan", [0.0, 235.0]);
        ui.pin("motion", [0.0, 310.0]);
        ui.pin("out", [472.0, 145.0]);
    }
    edits
}

pub fn visits(ui: &mut Ui, s: &Snapshot, selected: &mut Option<usize>) {
    let Some(run) = s.run() else {
        ui.label("No run");
        return;
    };
    let mut cells = Vec::new();
    for (i, v) in run.visits.iter().enumerate() {
        cells.extend([
            format!("{}", i + 1),
            format!("{}{}", v.station + 1, if v.station == run.zero { " / ZERO" } else { "" }),
            format!("{:.3}", v.reading.t_s),
            format!("{:.4}", v.reading.px),
            format!("{:.4}", v.reading.uncertainty_px),
            format!("{}", v.reading.used),
            v.reading.flags.names().collect::<Vec<_>>().join(", "),
        ]);
    }
    ui.table(
        "Raw visits",
        &["Visit", "Station", "Time / s", "Position / px", "Uncertainty / px", "Frames", "Checks"],
        &cells,
        selected,
    );
}

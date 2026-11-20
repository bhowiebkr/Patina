use measurement::{
    Reading,
    core::monitor::{Monitor, MonitorConfig, Readiness, RunPlan, Status},
};
use measurement_ui::{VIOLET, field, panel, place};
use patina_sdk::Ui;
#[patina_sdk::node(domain=frame,free_layout,color="#59366f")]
pub struct WarmUp {
    monitor: Monitor,
    key: Option<(u64, u64)>,
    status: Option<Status>,
    next: f64,
    run_s: f64,
    tolerance: f64,
    reset: u32,
    valid: bool,
    source: Option<Reading>,
    plan: Option<RunPlan>,
    values: Vec<f32>,
}
impl WarmUp {
    pub fn init() -> Self {
        Self {
            monitor: Monitor::new(MonitorConfig::default()),
            key: None,
            status: None,
            next: 0.0,
            run_s: 600.0,
            tolerance: 1.0,
            reset: 0,
            valid: false,
            source: None,
            plan: None,
            values: Vec::with_capacity(300),
        }
    }
    pub fn process(&mut self, reading: &Reading, run_s: f64, tolerance: f64, reset: u32) -> Result<String, String> {
        if !measurement::settings::range(run_s, 1.0, 14400.0) || !measurement::settings::range(tolerance, 0.001, 1000.0)
        {
            return Err("Invalid drift plan".into());
        }
        let key = reading.beam.frame.key();
        let plan = RunPlan { run_s, tolerance };
        let changed = self.source.as_ref().is_some_and(|s| {
            s.calibration.scale != reading.calibration.scale
                || s.beam.config != reading.beam.config
                || s.beam.frame.camera != reading.beam.frame.camera
                || s.beam.frame.orientation != reading.beam.frame.orientation
        });
        if reset != self.reset
            || key.zip(self.key).is_some_and(|(a, b)| a.0 != b.0)
            || self.plan != Some(plan)
            || changed
        {
            self.monitor = Monitor::new(MonitorConfig::default());
            self.status = None;
            self.next = 0.0;
        }
        self.plan = Some(plan);
        self.source = Some(reading.clone());
        self.reset = reset;
        self.run_s = run_s;
        self.tolerance = tolerance;
        self.valid = reading.position().is_some();
        if key != self.key
            && let (Some((v, _)), Some(im)) = (reading.position(), &reading.beam.frame.image)
        {
            self.monitor.push(im.t_s, v);
            if im.t_s >= self.next {
                self.status = Some(self.monitor.status(&RunPlan { run_s, tolerance }));
                self.next = im.t_s + 1.0;
            }
        }
        self.key = key;
        Ok(self.label())
    }
    fn label(&self) -> String {
        if !self.valid {
            return "No valid live reading".into();
        }
        match self.status.as_ref().map(|s| &s.readiness) {
            Some(Readiness::Ready) => "READY TO MEASURE".into(),
            Some(Readiness::Collecting { have_s, need_s }) => {
                format!("Collecting / {have_s:.0} s / need about {need_s:.0} s")
            }
            Some(Readiness::Noisy { noise }) => format!("NOISY / {noise:.3} um"),
            Some(Readiness::Settling { in_s, .. }) => format!("SETTLING / estimate {in_s:.0} s"),
            Some(Readiness::Drifting { rate_per_min }) => format!("DRIFTING / {rate_per_min:.3} um/min"),
            None => "Collecting drift history".into(),
        }
    }
    pub fn ui(&mut self, ui: &mut Ui) {
        panel(ui, [410.0, 394.0], "WARM-UP / DRIFT MONITOR", "SETTLE, NOISE, AND READINESS", VIOLET);
        place(ui, [16.0, 78.0]);
        ui.label(&self.label());
        let bins = self.monitor.bins();
        let origin = bins.first().map_or(0.0, |b| b.mean);
        self.values.clear();
        self.values.extend(bins.iter().rev().take(300).rev().map(|b| (b.mean - origin) as f32));
        let bound = self.values.iter().fold(0.1_f32, |v, x| v.max(x.abs()));
        let first = bins.get(bins.len().saturating_sub(300)).map_or(0.0, |b| b.t_s) as f32;
        place(ui, [16.0, 115.0]);
        ui.plot_axes(
            "Drift from first reading",
            &self.values,
            -bound..=bound,
            first..=bins.last().map_or(1.0, |b| b.t_s) as f32,
            ["s", "um"],
        );
        field(ui, [16.0, 285.0], "Planned run", "run_s", &mut self.run_s, 30.0, "s");
        field(ui, [16.0, 320.0], "Allowed drift", "tolerance", &mut self.tolerance, 0.1, "um");
        place(ui, [16.0, 357.0]);
        if ui.button("Restart estimate").clicked() {
            ui.set_param("reset", self.reset.wrapping_add(1));
        }
        ui.pin("reading", [0.0, 115.0]);
        ui.pin("out", [410.0, 115.0]);
    }
}

use measurement::{Plan, core::survey::Run};
use measurement_ui::{AMBER, field, panel, place, station_map};
use patina_sdk::Ui;
use std::sync::Arc;
#[patina_sdk::node(domain=frame,free_layout,color="#655219")]
pub struct StationPlan {
    plan: Plan,
    nx: u32,
    ny: u32,
    dx: f64,
    dy: f64,
    zero: u32,
    line: bool,
    built: Option<(u32, u32, f64, f64, u32, bool)>,
}
impl StationPlan {
    pub fn init() -> Self {
        Self { plan: Plan::default(), nx: 5, ny: 5, dx: 25.0, dy: 25.0, zero: 0, line: false, built: None }
    }
    pub fn process(&mut self, nx: u32, ny: u32, dx: f64, dy: f64, zero: u32, line: bool) -> Result<Plan, String> {
        if !(2..=64).contains(&nx)
            || !(2..=64).contains(&ny)
            || !measurement::settings::range(dx, 0.01, 10000.0)
            || !measurement::settings::range(dy, 0.01, 10000.0)
        {
            return Err("Use 2 to 64 stations per side and positive spacing".into());
        }
        if self.built != Some((nx, ny, dx, dy, zero, line)) {
            let mut run = if line {
                Run::line("Line survey", nx as usize, dx)
            } else {
                Run::grid("Surface survey", nx as usize, ny as usize, dx, dy)
            };
            run.set_zero(zero as usize).map_err(|e| e.to_string())?;
            self.plan = Plan(Arc::new(run));
            self.nx = nx;
            self.ny = ny;
            self.dx = dx;
            self.dy = dy;
            self.zero = zero;
            self.line = line;
            self.built = Some((nx, ny, dx, dy, zero, line));
        }
        Ok(self.plan.clone())
    }
    pub fn ui(&mut self, ui: &mut Ui) {
        panel(ui, [360.0, 526.0], "STATION PLAN", "LAYOUT AND ZERO REFERENCE", AMBER);
        place(ui, [16.0, 78.0]);
        if ui.toggle("Line survey", &mut self.line).changed() {
            ui.set_param("line", self.line);
        }
        place(ui, [16.0, 116.0]);
        if let Some(zero) = station_map(ui, &self.plan.0, self.zero as usize, [328.0, 190.0]) {
            ui.set_param("zero", zero);
        }
        field(ui, [16.0, 325.0], "Columns", "nx", &mut self.nx, 1, "");
        field(ui, [16.0, 360.0], "Rows", "ny", &mut self.ny, 1, "");
        field(ui, [16.0, 395.0], "X spacing", "dx", &mut self.dx, 1.0, "mm");
        field(ui, [16.0, 430.0], "Y spacing", "dy", &mut self.dy, 1.0, "mm");
        place(ui, [16.0, 479.0]);
        ui.label("Click the map to set zero. New run applies this plan.");
        ui.pin("out", [360.0, 210.0]);
    }
}

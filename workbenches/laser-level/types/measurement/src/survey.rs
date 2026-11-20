use crate::{
    Plan, Reading,
    acquisition::identity,
    core::{
        reading::{SampleRule, Sampler, Subsample},
        survey::{PixelGrid, Run, Session},
    },
    settings, storage,
};
use serde::{Deserialize, Serialize};
use std::{
    fmt,
    path::PathBuf,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Options {
    pub station: usize,
    pub rule: SampleRule,
    pub file: String,
}
impl Default for Options {
    fn default() -> Self {
        Self { station: 0, rule: SampleRule::default(), file: "measurements/manual.json".into() }
    }
}

/// A local controller model. It never sends machine commands.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Motion {
    pub simulated: bool,
    pub feed_mm_min: f64,
    pub lift_mm: f64,
    pub dwell_s: f64,
    pub zero_every: usize,
    pub object: u32,
    pub relief_um: f64,
}
impl patina_sdk::Viewer for Motion {
    fn text(&self, out: &mut patina_sdk::Text<'_>) -> fmt::Result {
        use fmt::Write;
        out.write_str(if self.simulated {
            "Simulated motion / no machine connection"
        } else {
            "Manual station movement"
        })
    }
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub session: Arc<Session>,
    pub run: usize,
    pub station: usize,
    pub phase: String,
    pub error: String,
    pub progress: f32,
    pub sample: Option<crate::core::reading::Reading>,
    pub pending: bool,
    pub active: bool,
    pub token: u64,
    pub sequence: u64,
    pub options: Options,
    pub position: [f64; 3],
}
impl Snapshot {
    pub fn command(&self, verb: &str) -> String {
        format!("{}:{}:{verb}", self.token, self.sequence + 1)
    }
    pub fn run(&self) -> Option<&Run> {
        self.session.runs.get(self.run)
    }
}
impl Default for Snapshot {
    fn default() -> Self {
        Self {
            session: Arc::new(Session::new(crate::core::scale::Scale::from_pixel_pitch(3.0))),
            run: 0,
            station: 0,
            phase: "Ready / choose a station".into(),
            error: String::new(),
            progress: 0.0,
            sample: None,
            pending: false,
            active: false,
            token: identity(),
            sequence: 0,
            options: Options::default(),
            position: [0.0; 3],
        }
    }
}
impl patina_sdk::Viewer for Snapshot {
    fn text(&self, out: &mut patina_sdk::Text<'_>) -> fmt::Result {
        use fmt::Write;
        write!(out, "{} / {} visits", self.phase, self.run().map_or(0, |r| r.visits.len()))
    }
}

struct Sampling {
    sampler: Sampler,
    station: usize,
    grid: PixelGrid,
    source: Reading,
    start: f64,
    automatic: bool,
    rule: SampleRule,
}
struct Move {
    station: usize,
    until: f64,
    start: f64,
    from: [f64; 3],
    to: [f64; 3],
    motion: Motion,
}

impl Move {
    fn position(&self, time: f64) -> ([f64; 3], &'static str) {
        let elapsed = (time - self.start).max(0.0);
        let speed = self.motion.feed_mm_min / 60.0;
        let lift = self.motion.lift_mm / speed;
        let travel = (self.to[0] - self.from[0]).hypot(self.to[1] - self.from[1]) / speed;
        if elapsed < lift {
            return ([self.from[0], self.from[1], elapsed * speed], "lifting");
        }
        if elapsed < lift + travel {
            let t = (elapsed - lift) / travel;
            return (
                [
                    self.from[0] + (self.to[0] - self.from[0]) * t,
                    self.from[1] + (self.to[1] - self.from[1]) * t,
                    self.motion.lift_mm,
                ],
                "travelling",
            );
        }
        if elapsed < 2.0 * lift + travel {
            return ([self.to[0], self.to[1], self.motion.lift_mm - (elapsed - lift - travel) * speed], "lowering");
        }
        (self.to, "settling")
    }
}

pub struct Survey {
    pub shown: Snapshot,
    sample: Option<Sampling>,
    pending: Option<(usize, PixelGrid, Vec<Subsample>)>,
    movement: Option<Move>,
    route: Vec<usize>,
    cursor: usize,
    key: Option<(u64, u64)>,
    generation: u64,
    offset: f64,
    placed: Option<usize>,
    loaded: bool,
    path: PathBuf,
    motion: Motion,
}
impl Default for Survey {
    fn default() -> Self {
        Self {
            shown: Snapshot::default(),
            sample: None,
            pending: None,
            movement: None,
            route: Vec::new(),
            cursor: 0,
            key: None,
            generation: 0,
            offset: 0.0,
            placed: None,
            loaded: false,
            path: PathBuf::new(),
            motion: Motion::default(),
        }
    }
}
impl Survey {
    pub fn update(
        &mut self,
        reading: &Reading,
        plan: &Plan,
        motion: &Motion,
        options: &str,
        request: &str,
    ) -> Result<Snapshot, String> {
        let opts: Options = settings::decode(options)?;
        settings::sample(opts.rule)?;
        if !settings::range(motion.dwell_s, 0.0, 60.0)
            || motion.object > 1
            || (motion.object == 1 && !settings::range(motion.relief_um, 1.0, 1500.0))
            || (motion.simulated
                && (!settings::range(motion.feed_mm_min, 1.0, 6000.0) || !settings::range(motion.lift_mm, 0.0, 100.0)))
        {
            return Err("Invalid simulated motion settings".into());
        }
        if opts.file.trim().is_empty() {
            return Err("Choose a session file".into());
        }
        if !self.loaded || opts.file != self.shown.options.file {
            if self.shown.active || self.pending.is_some() {
                return Err("Keep or discard the sample before opening another session".into());
            }
            let path = patina_sdk::project_path(&opts.file);
            let mut session = if path.exists() {
                storage::load(&path)?
            } else {
                Session::new(crate::core::scale::Scale::from_pixel_pitch(3.0))
            };
            if session.started_unix == 0.0 {
                let last = session.runs.iter().flat_map(|r| &r.visits).map(|v| v.reading.t_s).fold(0.0, f64::max);
                session.started_unix = unix_time() - last;
                if last > 0.0 {
                    session.runs.push((*plan.0).clone());
                }
            }
            self.shown.run = session.runs.len().saturating_sub(1);
            self.shown.session = Arc::new(session);
            self.path = path;
            self.generation = 0;
            self.key = None;
            self.placed = None;
            self.loaded = true;
        }
        self.shown.options = opts;
        if self.shown.session.runs.is_empty() {
            Arc::make_mut(&mut self.shown.session).runs.push((*plan.0).clone());
        }
        let frame = &reading.beam.frame;
        if frame.generation != self.generation {
            let was_active = self.shown.active;
            self.stop_active();
            if was_active {
                self.shown.error = "Camera restarted. The active sample was discarded.".into();
            }
            self.generation = frame.generation;
            let last =
                self.shown.session.runs.iter().flat_map(|r| &r.visits).map(|v| v.reading.t_s).fold(0.0, f64::max);
            self.offset = (unix_time() - self.shown.session.started_unix).max(last + 0.001)
                - frame.image.as_ref().map_or(0.0, |im| im.t_s);
            self.placed = None;
        }
        if self.motion != *motion {
            if self.shown.active {
                self.stop_active();
                self.shown.error = "Controller settings changed. The scan stopped.".into();
            }
            self.motion = *motion;
            self.placed = None;
        }
        let time = frame.image.as_ref().map_or(self.offset, |im| im.t_s + self.offset);
        if let Some(verb) = self.accept(request) {
            self.shown.error.clear();
            if let Err(e) = self.action(&verb, reading, plan, motion, time) {
                self.shown.error = e;
            }
        }
        if self.sample.is_none() && self.route.is_empty() && self.pending.is_none() {
            let count = self.shown.run().map_or(0, |r| r.stations.len());
            self.shown.station = self.shown.options.station.min(count.saturating_sub(1));
            self.place(reading, self.shown.station);
        }
        if self.shown.active && frame.image.is_none() {
            self.cancel();
            self.shown.error = "Camera stopped during the survey".into();
        }
        if self.key != frame.key() && frame.key().is_some() {
            self.key = frame.key();
            if let Some(m) = &self.movement {
                let part = ((time - m.start) / (m.until - m.start).max(0.001)).clamp(0.0, 1.0);
                self.shown.progress = part as f32;
                let (position, phase) = m.position(time);
                self.shown.position = position;
                self.shown.phase = format!("SIMULATION / {phase} / station {}", m.station + 1);
                if time >= m.until {
                    let station = m.station;
                    self.movement = None;
                    if let Err(e) = self.begin(reading, station, time, true) {
                        self.cancel();
                        self.shown.error = e;
                    }
                }
            }
            if let Err(e) = self.feed(reading, time) {
                self.stop_active();
                self.shown.error = e;
            }
            if !self.route.is_empty() && self.sample.is_none() && self.movement.is_none() && self.pending.is_none() {
                if self.cursor == self.route.len() {
                    self.route.clear();
                    self.shown.active = false;
                    self.shown.progress = 1.0;
                    self.shown.phase = "Simulated scan complete / closed on zero".into();
                } else {
                    self.move_to(reading, motion, time);
                }
            }
        }
        self.shown.pending = self.pending.is_some();
        Ok(self.shown.clone())
    }

    fn accept(&mut self, request: &str) -> Option<String> {
        let mut parts = request.split(':');
        let token = parts.next()?.parse::<u64>().ok()?;
        let seq = parts.next()?.parse::<u64>().ok()?;
        let verb = parts.next()?;
        if parts.next().is_some() || token != self.shown.token || seq <= self.shown.sequence {
            return None;
        }
        self.shown.sequence = seq;
        Some(verb.into())
    }
    fn action(&mut self, verb: &str, reading: &Reading, plan: &Plan, motion: &Motion, time: f64) -> Result<(), String> {
        match verb {
            "sample" if !self.shown.active && self.pending.is_none() => {
                self.begin(reading, self.shown.station, time, false)
            }
            "keep" => self.keep(),
            "cancel" => {
                self.cancel();
                Ok(())
            }
            "save" => storage::save(&self.path, &self.shown.session.to_json()),
            "new" if !self.shown.active && self.pending.is_none() => {
                if self.shown.session.runs.len() >= 256 {
                    return Err("Session has 256 runs. Open a new session file.".into());
                }
                let mut session = (*self.shown.session).clone();
                session.runs.push((*plan.0).clone());
                storage::save(&self.path, &session.to_json())?;
                self.shown.run = session.runs.len() - 1;
                self.shown.session = Arc::new(session);
                self.placed = None;
                Ok(())
            }
            "start" if !self.shown.active && self.pending.is_none() => {
                if !motion.simulated || reading.beam.frame.block.is_none() {
                    return Err("Automated Survey requires the simulated controller and synthetic camera".into());
                }
                if self
                    .shown
                    .run()
                    .is_some_and(|r| !r.visits.is_empty() || r.stations != plan.0.stations || r.zero != plan.0.zero)
                {
                    self.action("new", reading, plan, motion, time)?;
                }
                let run = &mut Arc::make_mut(&mut self.shown.session).runs[self.shown.run];
                run.name = if motion.object == 1 {
                    format!("Coin relief / scale {} um", motion.relief_um)
                } else {
                    "Surface plate".into()
                };
                let run = self.shown.run().ok_or("No station plan")?;
                self.route = route(run, motion.zero_every.max(1));
                self.cursor = 0;
                self.shown.active = true;
                self.motion = *motion;
                self.move_to(reading, motion, time);
                Ok(())
            }
            _ => Err("Action unavailable while a sample is active or awaits a decision".into()),
        }
    }
    fn cancel(&mut self) {
        self.stop_active();
        self.pending = None;
        self.shown.sample = None;
        self.shown.active = false;
        self.shown.pending = false;
        self.shown.progress = 0.0;
        self.shown.phase = "Idle / sample discarded".into();
    }
    fn stop_active(&mut self) {
        self.sample = None;
        self.movement = None;
        self.route.clear();
        self.shown.active = false;
        self.shown.phase =
            if self.pending.is_some() { "Sample retained / Keep or Discard" } else { "Ready / choose a station" }
                .into();
    }
    fn begin(&mut self, reading: &Reading, station: usize, time: f64, automatic: bool) -> Result<(), String> {
        if reading.position().is_none() {
            return Err("A valid calibrated beam is required".into());
        }
        let grid = reading.beam.frame.grid().ok_or("No camera frame")?;
        let run = self.shown.run().ok_or("No station plan")?;
        if station >= run.stations.len() {
            return Err("Station is outside the plan".into());
        }
        if run.grid_conflict(grid).is_some() {
            return Err("Camera grid changed. Start a new run.".into());
        }
        let mut expected = self.shown.session.scale;
        if expected.source == crate::core::scale::ScaleSource::PixelPitch {
            expected.um_per_px = self.shown.session.sensor.um_per_px(grid.width, grid.height);
        }
        if self.shown.session.runs.iter().any(|r| !r.visits.is_empty())
            && (expected != reading.calibration.scale.unwrap()
                || self.shown.session.sensor != reading.calibration.sensor)
        {
            return Err("Calibration changed. Start a new session.".into());
        }
        self.sample = Some(Sampling {
            sampler: Sampler::new(time),
            station,
            grid,
            source: reading.clone(),
            start: time,
            automatic,
            rule: self.shown.options.rule,
        });
        self.shown.station = station;
        self.shown.active = true;
        self.shown.sample = None;
        self.shown.progress = 0.0;
        self.shown.phase = format!("Sampling station {}", station + 1);
        Ok(())
    }
    fn feed(&mut self, reading: &Reading, time: f64) -> Result<(), String> {
        let Some(s) = &mut self.sample else { return Ok(()) };
        if self.shown.options.rule != s.rule
            || reading.beam.frame.grid() != Some(s.grid)
            || reading.beam.config != s.source.beam.config
            || reading.calibration.sensor != s.source.calibration.sensor
            || reading.calibration.scale != s.source.calibration.scale
            || reading.beam.frame.camera != s.source.beam.frame.camera
            || reading.beam.frame.orientation != s.source.beam.frame.orientation
        {
            return Err("Acquisition settings changed. The sample was discarded.".into());
        }
        if s.sampler.subsamples().len() >= 32768 {
            return Err("Sample exceeded its frame limit".into());
        }
        s.sampler.push(time, &reading.beam.fix);
        self.shown.progress = ((time - s.start) / self.shown.options.rule.max_s).clamp(0.0, 1.0) as f32;
        let scale = reading.calibration.scale.ok_or("No calibration")?;
        if !s.sampler.done(time, &self.shown.options.rule, scale.um_per_px) {
            return Ok(());
        }
        let s = self.sample.take().expect("sample exists");
        let sample = s.sampler.reading().ok_or("No beam in the sample")?;
        if sample.flags.few_frames {
            return Err("Too few valid frames. Repeat the sample.".into());
        }
        if self.shown.session.runs.iter().all(|r| r.visits.is_empty()) {
            let session = Arc::make_mut(&mut self.shown.session);
            session.scale = scale;
            session.sensor = reading.calibration.sensor.clone();
            session.finder = (*reading.beam.config).clone();
            session.camera = (*reading.beam.frame.camera).clone();
            session.view = Some(reading.beam.frame.orientation);
            session.sample = self.shown.options.rule;
        }
        self.shown.sample = Some(sample);
        self.shown.active = false;
        self.pending = Some((s.station, s.grid, s.sampler.finish()));
        self.shown.phase = "Sample ready / Keep or Discard".into();
        self.shown.progress = 1.0;
        if s.automatic {
            self.keep()?;
            self.shown.active = true;
        }
        Ok(())
    }
    fn keep(&mut self) -> Result<(), String> {
        let (station, grid, subs) = self.pending.as_ref().ok_or("No completed sample")?;
        let mut session = (*self.shown.session).clone();
        let run = session.runs.get_mut(self.shown.run).ok_or("No run")?;
        if run.visits.len() >= 65536 {
            return Err("Run has too many visits. Start a new run.".into());
        }
        run.use_grid(*grid).map_err(|e| e.to_string())?;
        run.record(*station, subs.clone()).map_err(|e| e.to_string())?;
        storage::save(&self.path, &session.to_json())?;
        self.shown.session = Arc::new(session);
        self.pending = None;
        self.shown.phase = "Sample saved".into();
        Ok(())
    }
    fn place(&mut self, reading: &Reading, station: usize) {
        if self.placed == Some(station) {
            return;
        }
        if let (Some(block), Some(scale), Some(run)) =
            (&reading.beam.frame.block, reading.calibration.scale, self.shown.run())
        {
            let sign = if scale.invert { -1.0 } else { 1.0 };
            let height = crate::specimen::height(run, station, self.motion.object, self.motion.relief_um);
            block.place(sign * height / scale.um_per_px);
            self.placed = Some(station);
        }
    }
    fn move_to(&mut self, reading: &Reading, motion: &Motion, time: f64) {
        let station = self.route[self.cursor];
        self.cursor += 1;
        let p = &self.shown.run().expect("active run").stations[station];
        let to = [p.x_mm, p.y_mm, 0.0];
        let from = self.shown.position;
        let distance = (to[0] - from[0]).hypot(to[1] - from[1]) + 2.0 * motion.lift_mm;
        self.place(reading, station);
        self.movement = Some(Move {
            station,
            start: time,
            until: time + 60.0 * distance / motion.feed_mm_min + motion.dwell_s,
            from,
            to,
            motion: *motion,
        });
        self.shown.station = station;
        self.shown.phase = format!("SIMULATION / move, lower, settle / station {}", station + 1);
    }
}

fn unix_time() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs_f64()
}

pub fn route(run: &Run, every: usize) -> Vec<usize> {
    let mut path = vec![run.zero];
    for station in 0..run.stations.len() {
        if station == run.zero {
            continue;
        }
        path.push(station);
        if station % every.max(1) == 0 {
            path.push(run.zero);
        }
    }
    if path.last() != Some(&run.zero) {
        path.push(run.zero);
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn route_checks_zero_and_commands_do_not_replay() {
        let r = Run::grid("test", 3, 2, 10.0, 10.0);
        let path = route(&r, 2);
        assert_eq!(path.first(), Some(&0));
        assert_eq!(path.last(), Some(&0));
        for station in 0..6 {
            assert!(path.contains(&station));
        }
        let mut s = Survey::default();
        let command = s.shown.command("sample");
        assert_eq!(s.accept(&command), Some("sample".into()));
        assert!(s.accept(&command).is_none());
        assert!(Survey::default().accept(&command).is_none());
    }
    #[test]
    fn movement_lifts_before_travel_and_lowers_before_settling() {
        let m = Move {
            station: 1,
            start: 0.0,
            until: 4.0,
            from: [0.0; 3],
            to: [10.0, 0.0, 0.0],
            motion: Motion { feed_mm_min: 600.0, lift_mm: 10.0, dwell_s: 1.0, ..Default::default() },
        };
        assert_eq!(m.position(0.5), ([0.0, 0.0, 5.0], "lifting"));
        assert_eq!(m.position(1.5), ([5.0, 0.0, 10.0], "travelling"));
        assert_eq!(m.position(2.5), ([10.0, 0.0, 5.0], "lowering"));
        assert_eq!(m.position(3.5), ([10.0, 0.0, 0.0], "settling"));
    }
}

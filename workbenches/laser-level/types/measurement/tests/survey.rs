use measurement::{
    Beam, Calibration, Frame, Plan, Reading, camera,
    core::{
        beam::Fix, camera::Sensor, frame::Orientation, reading::SampleRule, scale::Scale, survey::Run, synth::Surface,
    },
    results::Processor,
    settings, storage,
    survey::{Motion, Options, Survey},
};
use std::{fs, path::PathBuf, sync::Arc};

struct Rig {
    survey: Survey,
    plan: Plan,
    reading: Reading,
    options: Options,
    motion: Motion,
    step: u64,
    dir: PathBuf,
}
impl Rig {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("patina-survey-{:x}", measurement::acquisition::identity()));
        let options = Options {
            file: dir.join("session.json").to_string_lossy().into_owned(),
            rule: SampleRule { min_s: 0.2, max_s: 0.2, target_um: None },
            ..Default::default()
        };
        let sensor = Sensor { width_px: 640, height_px: 480, pixel_pitch_um: 3.0, ..Default::default() };
        let reading = Reading {
            beam: Beam {
                frame: Frame {
                    generation: 1,
                    block: Some(Default::default()),
                    orientation: Orientation::default(),
                    ..Default::default()
                },
                ..Default::default()
            },
            calibration: Calibration { sensor, scale: Some(Scale::from_pixel_pitch(3.0)) },
        };
        let mut rig = Self {
            survey: Survey::default(),
            plan: Plan(Arc::new(Run::grid("test", 3, 3, 1.0, 1.0))),
            reading,
            options,
            motion: Motion {
                simulated: true,
                feed_mm_min: 6000.0,
                lift_mm: 0.1,
                dwell_s: 0.1,
                zero_every: 3,
                ..Default::default()
            },
            step: 0,
            dir,
        };
        rig.tick("");
        rig
    }
    fn tick(&mut self, request: &str) {
        self.step += 1;
        let t = self.step as f64 / 30.0;
        let shift = self.reading.beam.frame.block.as_ref().unwrap().shift_px();
        self.reading.beam.frame.image =
            Some(Arc::new(camera::Frame { width: 640, height: 480, data: Vec::new(), t_s: t, seq: self.step }));
        self.reading.beam.fix = Ok(Fix {
            position_px: 320.0 + shift + 0.02 * t,
            uncertainty_px: 0.01,
            tilt: 0.0,
            fwhm_px: 30.0,
            peak: 150.0,
            snr: 100.0,
            strips_used: 16,
            warnings: Default::default(),
        });
        self.survey.update(&self.reading, &self.plan, &self.motion, &settings::encode(&self.options), request).unwrap();
    }
    fn command(&mut self, verb: &str) {
        let request = self.survey.shown.command(verb);
        self.tick(&request);
    }
    fn sample(&mut self) {
        self.command("sample");
        for _ in 0..10 {
            self.tick("");
        }
        assert!(self.survey.shown.pending, "{}", self.survey.shown.error);
    }
}
impl Drop for Rig {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn manual_sample_keeps_its_station_and_survives_camera_restart() {
    let mut rig = Rig::new();
    rig.sample();
    rig.options.station = 3;
    rig.reading.beam.frame.generation = 2;
    rig.tick("");
    assert!(rig.survey.shown.pending);
    rig.command("keep");
    let session = storage::load(&PathBuf::from(&rig.options.file)).unwrap();
    assert_eq!(session.runs[0].visits[0].station, 0);
    assert_eq!(rig.survey.shown.station, 3);
    let previous = rig.survey.shown.command("keep");
    let mut reopened = Survey::default();
    let shown =
        reopened.update(&rig.reading, &rig.plan, &rig.motion, &settings::encode(&rig.options), &previous).unwrap();
    assert_eq!(shown.run().unwrap().visits.len(), 1);
    assert!(!shown.active);
}

#[test]
fn automatic_scan_removes_linear_drift_and_saves_every_visit() {
    let mut rig = Rig::new();
    rig.command("start");
    for _ in 0..700 {
        if !rig.survey.shown.active {
            break;
        }
        rig.tick("");
    }
    let shown = &rig.survey.shown;
    assert!(!shown.active, "scan never finished");
    assert!(shown.error.is_empty(), "{}", shown.error);
    let run = shown.run().unwrap();
    assert_eq!(run.visits.last().unwrap().station, run.zero);
    let mut processor = Processor::default();
    let data = processor.update(shown, true);
    let results = data.result.unwrap();
    assert_eq!(results.points.len(), 9);
    for p in &results.points {
        assert!((p.measured_um - Surface::default().seen_um(run, p.station)).abs() < 1e-8, "{p:?}");
    }
    assert!(Arc::ptr_eq(&results, &processor.update(shown, true).result.unwrap()));
    assert_eq!(storage::load(&PathBuf::from(&rig.options.file)).unwrap(), *shown.session);
}

#[test]
fn a_failed_automatic_save_retains_the_completed_sample() {
    let mut rig = Rig::new();
    fs::create_dir_all(&rig.dir).unwrap();
    fs::create_dir(PathBuf::from(&rig.options.file)).unwrap();
    rig.command("start");
    for _ in 0..50 {
        rig.tick("");
        if rig.survey.shown.pending {
            break;
        }
    }
    assert!(rig.survey.shown.pending);
    assert!(!rig.survey.shown.active);
    assert!(!rig.survey.shown.error.is_empty());
    assert!(rig.survey.shown.run().unwrap().visits.is_empty());
    fs::remove_dir(&rig.options.file).unwrap();
    rig.command("keep");
    assert_eq!(storage::load(&PathBuf::from(&rig.options.file)).unwrap().runs[0].visits.len(), 1);
}

#[test]
fn changed_acquisition_or_sampling_settings_discard_the_active_sample() {
    let mut rig = Rig::new();
    rig.command("sample");
    rig.options.rule.max_s = 0.3;
    rig.tick("");
    assert!(!rig.survey.shown.active);
    assert!(!rig.survey.shown.pending);
    assert!(rig.survey.shown.error.contains("settings changed"));
    rig.command("sample");
    rig.reading.calibration.scale.as_mut().unwrap().um_per_px = 4.0;
    rig.tick("");
    assert!(!rig.survey.shown.active);
    assert!(!rig.survey.shown.pending);
}

#[test]
fn coin_scan_preserves_relief_and_applies_a_changed_empty_plan() {
    let mut rig = Rig::new();
    rig.plan = Plan(Arc::new(Run::grid("Coin", 9, 9, 3.75, 3.75)));
    rig.motion.object = 1;
    rig.motion.relief_um = 800.0;
    rig.motion.zero_every = 9;
    rig.command("start");
    let mut processor = Processor::default();
    let mut early = false;
    for _ in 0..5000 {
        rig.tick("");
        let data = processor.update(&rig.survey.shown, true);
        if data.result.is_none() && data.surface.as_ref().is_some_and(|s| s.measured() > 1) {
            early = true;
        }
        if !rig.survey.shown.active {
            break;
        }
    }
    assert!(early, "raw heights must appear before a plane can fit");
    assert!(!rig.survey.shown.active);
    assert!(rig.survey.shown.error.is_empty(), "{}", rig.survey.shown.error);
    let data = processor.update(&rig.survey.shown, true);
    let surface = data.surface.unwrap();
    assert_eq!(surface.measured(), 81);
    for (xy, h) in surface.xy.iter().zip(&surface.heights) {
        let expected = measurement::specimen::coin(xy[0], xy[1]) * 800.0;
        assert!((h.unwrap()[1] - expected).abs() < 1e-7);
    }
    assert!(surface.range(1)[1] > 750.0);
}

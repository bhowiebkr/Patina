//! Runs the actual camera, beam finder, sampler and disk workflow without a GUI.
use measurement::{
    Calibration, Plan, Reading,
    acquisition::{Camera, Finder},
    core::{camera::Sensor, frame::Orientation, reading::SampleRule, scale::Scale, survey::Run},
    results::Processor,
    settings, specimen,
    survey::{Motion, Options, Survey},
};
use std::{path::PathBuf, sync::Arc, time::Instant};

fn main() -> Result<(), String> {
    let file = PathBuf::from(std::env::args().nth(1).ok_or("Pass an unused output session path")?);
    if file.exists() {
        return Err("The output file already exists".into());
    }
    let size = std::env::args().nth(2).unwrap_or("41".into()).parse::<usize>().map_err(|e| e.to_string())?;
    if !(3..=64).contains(&size) {
        return Err("Use 3 to 64 stations per side".into());
    }
    let plan = Plan(Arc::new(Run::grid("Coin relief", size, size, 30.0 / (size - 1) as f64, 30.0 / (size - 1) as f64)));
    let motion = Motion {
        simulated: true,
        feed_mm_min: 3000.,
        lift_mm: 0.5,
        dwell_s: 0.05,
        zero_every: size,
        object: 1,
        relief_um: 800.,
    };
    let options = settings::encode(&Options {
        file: file.to_string_lossy().into_owned(),
        rule: SampleRule { min_s: 0.2, max_s: 0.5, target_um: None },
        ..Default::default()
    });
    let mut camera = Camera::default();
    let mut finder = Finder::default();
    let mut survey = Survey::default();
    let sensor = Sensor::default();
    let mut scale = Scale::from_pixel_pitch(sensor.um_per_px(640, 480));
    scale.invert = false;
    let calibration = Calibration { sensor, scale: Some(scale) };
    let mut request = String::new();
    let start = Instant::now();
    let mut frames = 0;
    loop {
        let mut frame = camera.update(r#"{"mode":{"width":640,"height":480,"fps":30,"format":"yuyv"}}"#, true)?;
        frame.orientation = Orientation { quarter_turns: 1, mirrored: false };
        let reading = Reading { beam: finder.update(&frame, "")?, calibration: calibration.clone() };
        let shown = survey.update(&reading, &plan, &motion, &options, &request)?;
        frames += 1;
        if !shown.error.is_empty() {
            return Err(shown.error);
        }
        if frames == 1 {
            request = shown.command("start");
        } else {
            request.clear();
        }
        if frames % 2000 == 0 {
            println!(
                "{frames} frames / {} saved visits / {:.1} seconds",
                shown.run().unwrap().visits.len(),
                start.elapsed().as_secs_f64()
            );
        }
        if frames > 2 && !shown.active {
            break;
        }
        if frames > size * size * 40 {
            return Err("Scan exceeded its frame budget".into());
        }
    }
    let data = Processor::default().update(&survey.shown, true);
    let surface = data.surface.ok_or(data.error)?;
    if surface.measured() != size * size {
        return Err("Missing measured stations".into());
    }
    let mut squared = 0.;
    let mut max = 0.0_f64;
    for (xy, h) in surface.xy.iter().zip(&surface.heights) {
        let error = h.unwrap()[1] - 800. * specimen::coin(xy[0], xy[1]);
        squared += error * error;
        max = max.max(error.abs());
    }
    let rms = (squared / (size * size) as f64).sqrt();
    println!(
        "{} stations / {frames} frames / {:.2} ms per frame / RMS {rms:.3} um / maximum {max:.3} um",
        surface.measured(),
        start.elapsed().as_secs_f64() * 1000. / frames as f64
    );
    if max > 5.0 {
        return Err("Sensor reconstruction exceeded 5 um error".into());
    }
    Ok(())
}

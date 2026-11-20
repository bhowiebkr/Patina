use crate::{
    Beam, Frame,
    camera::{
        self, FrameSource,
        capture::{Capture, Command, Event},
    },
    core::{
        beam,
        camera::{CameraSettings, ControlInfo, VideoMode},
    },
    settings,
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub fn identity() -> u64 {
    static SERIAL: AtomicU64 = AtomicU64::new(1);
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos() as u64
        ^ SERIAL.fetch_add(1, Ordering::Relaxed)
}

pub struct Camera {
    synthetic: Option<camera::synthetic::Synthetic>,
    capture: Option<Capture>,
    saved: Option<CameraSettings>,
    received: Instant,
    pub frame: Frame,
    pub controls: Vec<ControlInfo>,
    pub modes: Vec<VideoMode>,
    pub status: String,
    pub dropped: u64,
    pub refused: Vec<String>,
}
impl Default for Camera {
    fn default() -> Self {
        Self {
            synthetic: None,
            capture: None,
            saved: None,
            received: Instant::now(),
            frame: Frame::default(),
            controls: Vec::new(),
            modes: Vec::new(),
            status: "Stopped".into(),
            dropped: 0,
            refused: Vec::new(),
        }
    }
}
impl Camera {
    pub fn update(&mut self, text: &str, enabled: bool) -> Result<Frame, String> {
        let mut settings = settings::camera(text)?;
        settings.device.get_or_insert_with(|| camera::synthetic::NAME.into());
        if !enabled {
            self.stop();
            return Ok(self.frame.clone());
        }
        let reopen = self.saved.as_ref().is_none_or(|old| old.device != settings.device || old.mode != settings.mode);
        if reopen {
            self.stop();
            self.frame.generation = identity();
            if settings.device.as_deref() == Some(camera::synthetic::NAME) {
                let mut source = camera::synthetic::Synthetic::open(settings.mode);
                source.paced = false;
                let block = camera::synthetic::Block::default();
                source.block = Some(block.clone());
                self.frame.block = Some(block);
                self.modes = source.modes().to_vec();
                self.synthetic = Some(source);
            } else {
                self.capture = Some(Capture::start(settings.clone()).map_err(|e| e.to_string())?);
                self.status = "Opening camera".into();
            }
            self.received = Instant::now();
        }
        if self.saved.as_ref() != Some(&settings) {
            self.refused.clear();
            if let Some(s) = &mut self.synthetic {
                for a in camera::apply(s, &settings.controls) {
                    if !a.held() {
                        self.refused.push(format!("{} was not held", a.setting.id.label()));
                    }
                }
                self.controls = s.controls();
            }
            if let Some(c) = &self.capture {
                c.send(Command::Apply(settings.controls.clone()));
            }
            self.frame.camera = Arc::new(settings.clone());
            self.saved = Some(settings);
        }
        if let Some(s) = &mut self.synthetic {
            self.frame.image = Some(Arc::new(s.next_frame().map_err(|e| e.to_string())?));
            self.status = "Synthetic camera / simulated sensor".into();
            self.received = Instant::now();
        }
        if let Some(c) = &self.capture {
            for event in c.try_iter() {
                match event {
                    Event::Opened { modes, controls, applied, .. } => {
                        self.modes = modes;
                        self.controls = controls;
                        for a in applied {
                            if !a.held() {
                                self.refused.push(format!("{} was not held", a.setting.id.label()));
                            }
                        }
                        self.status = "Camera open / waiting for frame".into();
                    }
                    Event::Frame(f) => {
                        self.frame.image = Some(Arc::new(f));
                        self.received = Instant::now();
                        self.status = "Live camera".into();
                    }
                    Event::Control(a) => {
                        if !a.held() {
                            self.refused.push(format!("{} was not held", a.setting.id.label()));
                        }
                        if let Ok(info) = a.result
                            && let Some(old) = self.controls.iter_mut().find(|c| c.id == info.id)
                        {
                            *old = info;
                        }
                    }
                    Event::Controls(c) => self.controls = c,
                    Event::Error(e) => {
                        self.status = e.to_string();
                        self.frame.image = None;
                    }
                }
            }
            self.dropped = c.dropped();
            if self.received.elapsed() > Duration::from_secs(3) {
                self.frame.image = None;
                self.status = "No recent camera frame".into();
            }
        }
        Ok(self.frame.clone())
    }
    fn stop(&mut self) {
        self.synthetic = None;
        self.capture = None;
        self.saved = None;
        self.frame = Frame::default();
        self.controls.clear();
        self.modes.clear();
        self.status = "Stopped".into();
    }
}

#[derive(Default)]
pub struct Finder {
    pub output: Beam,
    key: Option<(u64, u64)>,
    settings: String,
}
impl Finder {
    pub fn update(&mut self, frame: &Frame, text: &str) -> Result<Beam, String> {
        if frame.key().is_some()
            && self.key == frame.key()
            && self.settings == text
            && self.output.frame.orientation == frame.orientation
        {
            return Ok(self.output.clone());
        }
        let mut cfg = settings::finder(text)?;
        cfg.axis = frame.orientation.axis();
        if let Some(im) = &frame.image {
            if let Some(roi) = cfg.roi
                && roi.clamped(im.width, im.height) != Some(roi)
            {
                return Err("ROI must stay inside the frame".into());
            }
            let luma = im.luma().map_err(|e| e.to_string())?;
            self.output = Beam {
                frame: frame.clone(),
                fix: beam::find(&luma, &cfg),
                detail: Arc::new(beam::inspect(&luma, &cfg)),
                config: Arc::new(cfg),
            };
        } else {
            self.output = Beam { frame: frame.clone(), ..Beam::default() };
        }
        self.key = frame.key();
        self.settings = text.into();
        Ok(self.output.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn synthetic_camera_finder_and_stop() {
        let mut camera = Camera::default();
        let mut finder = Finder::default();
        let settings = r#"{"mode":{"width":640,"height":480,"fps":30.0,"format":"yuyv"}}"#;
        let frame = camera.update(settings, true).unwrap();
        let beam = finder.update(&frame, "").unwrap();
        assert!(beam.fix.is_ok(), "{:?}", beam.fix);
        assert!(Arc::ptr_eq(&beam.detail, &finder.update(&frame, "").unwrap().detail));
        assert!(camera.update(settings, false).unwrap().image.is_none());
        assert!(finder.update(&camera.frame, "").unwrap().fix.is_err());
    }
}

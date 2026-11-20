//! Laser Level Webcam adapted to graph values. All measurement logic stays in this workbench.
pub mod acquisition;
pub mod results;
pub mod settings;
pub mod storage;
pub mod survey;

use core::{
    beam::{Fix, Inspection, NoBeam},
    camera::Sensor,
    frame::Orientation,
    scale::Scale,
    survey::PixelGrid,
};
pub use laser_camera as camera;
pub use laser_core as core;
use std::{fmt, sync::Arc};

#[derive(Clone, Default)]
pub struct Frame {
    pub image: Option<Arc<camera::Frame>>,
    pub generation: u64,
    pub orientation: Orientation,
    pub block: Option<camera::synthetic::Block>,
    pub camera: Arc<core::camera::CameraSettings>,
}
impl fmt::Debug for Frame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.image {
            Some(im) => write!(f, "{} x {} / frame {} / {:.3} s", im.width, im.height, im.seq, im.t_s),
            None => f.write_str("No camera frame"),
        }
    }
}
impl Frame {
    pub fn key(&self) -> Option<(u64, u64)> {
        self.image.as_ref().map(|im| (self.generation, im.seq))
    }
    pub fn grid(&self) -> Option<PixelGrid> {
        self.image.as_ref().map(|im| PixelGrid {
            width: im.width as u32,
            height: im.height as u32,
            axis: self.orientation.axis(),
        })
    }
}

#[derive(Clone, Debug)]
pub struct Beam {
    pub frame: Frame,
    pub fix: Result<Fix, NoBeam>,
    pub detail: Arc<Inspection>,
    pub config: Arc<core::beam::FinderConfig>,
}
impl Default for Beam {
    fn default() -> Self {
        Self {
            frame: Frame::default(),
            fix: Err(NoBeam::Flat),
            detail: Arc::new(Inspection { offset_px: 0, profile: Vec::new(), zones: Err(NoBeam::Flat) }),
            config: Arc::new(Default::default()),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Calibration {
    pub sensor: Sensor,
    pub scale: Option<Scale>,
}

#[derive(Clone, Debug, Default)]
pub struct Reading {
    pub beam: Beam,
    pub calibration: Calibration,
}
impl Reading {
    pub fn position(&self) -> Option<(f64, f64)> {
        let fix = self.beam.fix.as_ref().ok()?;
        let scale = self.calibration.scale.as_ref()?;
        Some((scale.um(fix.position_px), fix.uncertainty_px * scale.um_per_px))
    }
}

#[derive(Clone, Debug)]
pub struct Plan(pub Arc<core::survey::Run>);
impl Default for Plan {
    fn default() -> Self {
        Self(Arc::new(core::survey::Run::grid("Surface", 5, 5, 25.0, 25.0)))
    }
}

#[derive(Clone, Debug, Default)]
pub struct ResultData {
    pub result: Option<Arc<core::results::Results>>,
    pub name: String,
    pub error: String,
    pub correction: core::survey::DriftCorrection,
}

macro_rules! viewer {
    ($($t:ty),*) => { $(impl patina_sdk::Viewer for $t {
        fn text(&self, out: &mut patina_sdk::Text<'_>) -> fmt::Result {
            use fmt::Write;
            write!(out, "{self:?}")
        }
    })* };
}
viewer!(Frame, Calibration, Plan, ResultData);
impl patina_sdk::Viewer for Beam {
    fn text(&self, out: &mut patina_sdk::Text<'_>) -> fmt::Result {
        use fmt::Write;
        match &self.fix {
            Ok(f) => write!(out, "{:.3} px +/- {:.3} / {} strips", f.position_px, f.uncertainty_px, f.strips_used),
            Err(e) => out.write_str(e.text()),
        }
    }
}
impl patina_sdk::Viewer for Reading {
    fn text(&self, out: &mut patina_sdk::Text<'_>) -> fmt::Result {
        use fmt::Write;
        match self.position() {
            Some((v, u)) => write!(out, "{v:.3} +/- {u:.3} um / absolute beam position"),
            None => out.write_str("No valid reading"),
        }
    }
}

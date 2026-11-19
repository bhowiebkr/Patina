use forge_core::{AssetInfo, FittedView};
use glam::{Mat3, Mat4, Quat, Vec3};

/// Turntable camera around `target`. The turntable frame is Y up, turned by `tilt`, so a fitted
/// view's roll stays while orbiting.
#[derive(Clone, Copy, Debug)]
pub struct OrbitCamera {
    pub target: Vec3,
    /// Radians about the frame's +Y; 0 looks from +Z.
    pub yaw: f32,
    /// Radians above the frame's horizon.
    pub pitch: f32,
    pub distance: f32,
    pub fov_y: f32,
    pub tilt: Quat,
}

impl OrbitCamera {
    /// The web viewer's 3/4 view, distance fitted to a bounding sphere.
    pub fn framing(center: Vec3, radius: f32) -> Self {
        let dir = Vec3::new(1.5, 1.2, -1.9).normalize();
        let mut camera = Self {
            target: center,
            yaw: dir.x.atan2(dir.z),
            pitch: dir.y.asin(),
            distance: 1.0,
            fov_y: 30f32.to_radians(),
            tilt: Quat::IDENTITY,
        };
        camera.fit(radius);
        camera
    }

    /// The camera a reference render was fitted with (the CAD pipeline's fit.json).
    pub fn fitted(view: &FittedView) -> Self {
        let dir = Vec3::from(view.direction).normalize();
        let up = Vec3::from(view.up);
        let up = (up - dir * up.dot(dir)).normalize();
        // The frame's +Z is the view direction and +Y the image's up: yaw = pitch = 0.
        let tilt = Quat::from_mat3(&Mat3::from_cols(up.cross(dir), up, dir));
        Self {
            target: view.target.into(),
            yaw: 0.0,
            pitch: 0.0,
            distance: view.distance,
            fov_y: view.fov_y.to_radians(),
            tilt,
        }
    }

    /// The asset's fitted view if it has one, else the 3/4 framing. The fitted view is moved in
    /// so the asset fills the preview (the reference render leaves a wide margin).
    pub fn for_asset(info: &AssetInfo) -> Self {
        match &info.view {
            Some(view) => {
                let mut camera = Self::fitted(view);
                camera.zoom(0.9);
                camera
            }
            None => Self::framing(info.bounds.center(), info.bounds.radius()),
        }
    }

    pub fn fit(&mut self, radius: f32) {
        self.distance = radius.max(1e-3) / (self.fov_y * 0.5).sin() * 1.05;
    }

    /// Unit vector from the target to the eye.
    pub fn direction(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        self.tilt * Vec3::new(cp * sy, sp, cp * cy)
    }

    pub fn eye(&self) -> Vec3 {
        self.target + self.direction() * self.distance
    }

    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        let proj = Mat4::perspective_rh(
            self.fov_y,
            aspect.max(1e-3),
            self.distance * 0.02,
            self.distance * 20.0,
        );
        proj * Mat4::look_at_rh(self.eye(), self.target, self.tilt * Vec3::Y)
    }

    pub fn orbit(&mut self, dx: f32, dy: f32) {
        self.yaw -= dx;
        self.pitch = (self.pitch + dy).clamp(-1.55, 1.55);
    }

    pub fn zoom(&mut self, factor: f32) {
        self.distance = (self.distance * factor).max(1e-3);
    }

    /// Moves the target so the scene follows a drag of (dx, dy) pixels in a view `height` pixels tall.
    pub fn pan(&mut self, dx: f32, dy: f32, height: f32) {
        let dir = self.direction();
        let right = (self.tilt * Vec3::Y).cross(dir).normalize_or_zero();
        let up = dir.cross(right);
        let scale = 2.0 * self.distance * (self.fov_y * 0.5).tan() / height.max(1.0);
        self.target += (up * dy - right * dx) * scale;
    }
}

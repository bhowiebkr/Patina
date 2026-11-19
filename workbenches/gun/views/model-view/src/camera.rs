//! The orbit camera of the 3D view. Z is up.

/// The vertical field of view, in radians.
const FOV: f32 = 0.8;

/// A camera that turns around a target point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub target: [f32; 3],
    /// The turn around Z, in radians.
    pub yaw: f32,
    /// The angle above the XY plane, in radians.
    pub pitch: f32,
    pub distance: f32,
}

impl Default for Camera {
    fn default() -> Camera {
        Camera { target: [0.0; 3], yaw: -1.0, pitch: 0.45, distance: 6.0 }
    }
}

impl Camera {
    /// The position of the camera.
    pub fn eye(&self) -> [f32; 3] {
        let ((sy, cy), (sp, cp)) = (self.yaw.sin_cos(), self.pitch.sin_cos());
        add(self.target, scale([cp * cy, cp * sy, sp], self.distance))
    }

    /// Turns the camera by a pointer move in pixels.
    pub fn orbit(&mut self, [dx, dy]: [f32; 2]) {
        self.yaw -= dx * 0.008;
        // The limit keeps the view direction away from the up axis.
        self.pitch = (self.pitch + dy * 0.008).clamp(-1.55, 1.55);
    }

    /// Moves the target with a pointer move in pixels, in a viewport of `height` pixels.
    pub fn pan(&mut self, [dx, dy]: [f32; 2], height: f32) {
        let [right, up, _] = self.axes();
        let per_pixel = 2.0 * self.distance * (FOV / 2.0).tan() / height.max(1.0);
        self.target = add(self.target, add(scale(right, -dx * per_pixel), scale(up, dy * per_pixel)));
    }

    /// Changes the distance by a wheel move in pixels. Down is positive and moves away.
    pub fn zoom(&mut self, wheel: f32) {
        self.distance = (self.distance * (wheel * 0.001).clamp(-1.0, 1.0).exp()).clamp(1e-3, 1e6);
    }

    /// Puts the center of the bounds at the target, at a distance that shows all of the bounds.
    pub fn frame(&mut self, [low, high]: [[f32; 3]; 2]) {
        let size = sub(high, low);
        let radius = (dot(size, size).sqrt() / 2.0).max(1e-3);
        self.target = scale(add(low, high), 0.5);
        self.distance = radius / (FOV / 2.0).sin() * 1.1;
    }

    /// The right, up and forward axes of the view.
    fn axes(&self) -> [[f32; 3]; 3] {
        let forward = normalize(sub(self.target, self.eye()));
        let right = normalize(cross(forward, [0.0, 0.0, 1.0]));
        [right, cross(right, forward), forward]
    }

    /// The matrix from the world to the clip space for `aspect` = width / height, as columns.
    /// The depth goes from 0 at the near plane to 1 at the far plane.
    pub fn matrix(&self, aspect: f32) -> [[f32; 4]; 4] {
        let (eye, [r, u, f]) = (self.eye(), self.axes());
        let view = [
            [r[0], u[0], -f[0], 0.0],
            [r[1], u[1], -f[1], 0.0],
            [r[2], u[2], -f[2], 0.0],
            [-dot(r, eye), -dot(u, eye), dot(f, eye), 1.0],
        ];
        let (near, far) = (self.distance * 0.01, self.distance * 1000.0);
        let t = 1.0 / (FOV / 2.0).tan();
        let depth = far / (near - far);
        let projection = [[t / aspect.max(1e-3), 0.0, 0.0, 0.0], [0.0, t, 0.0, 0.0], [0.0, 0.0, depth, -1.0], [0.0, 0.0, near * depth, 0.0]];
        view.map(|column| std::array::from_fn(|row| (0..4).map(|k| projection[k][row] * column[k]).sum()))
    }

    /// The direction to the key light: from the camera, a little up and to the left.
    pub fn light(&self) -> [f32; 3] {
        let [right, up, forward] = self.axes();
        normalize(add(scale(forward, -1.0), add(scale(up, 0.6), scale(right, -0.4))))
    }
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn scale(a: [f32; 3], s: f32) -> [f32; 3] {
    a.map(|v| v * s)
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn normalize(a: [f32; 3]) -> [f32; 3] {
    scale(a, 1.0 / dot(a, a).sqrt().max(1e-12))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The clip position of `p` after the divide by w.
    fn clip(camera: &Camera, p: [f32; 3]) -> [f32; 3] {
        let m = camera.matrix(1.5);
        let v: [f32; 4] = std::array::from_fn(|row| m[0][row] * p[0] + m[1][row] * p[1] + m[2][row] * p[2] + m[3][row]);
        [v[0] / v[3], v[1] / v[3], v[2] / v[3]]
    }

    #[test]
    fn the_frame_shows_all_corners_of_the_bounds() {
        let mut camera = Camera::default();
        let bounds = [[-1.2, -1.2, 0.0], [1.2, 1.2, 2.9]];
        camera.frame(bounds);
        let center = clip(&camera, camera.target);
        assert!(center[0].abs() < 1e-4 && center[1].abs() < 1e-4 && center[2] > 0.0 && center[2] < 1.0, "{center:?}");
        for i in 0..8 {
            let corner = std::array::from_fn(|axis| bounds[(i >> axis) & 1][axis]);
            let [x, y, z] = clip(&camera, corner);
            assert!(x.abs() < 1.0 && y.abs() < 1.0 && z > 0.0 && z < 1.0, "{corner:?} gives {x} {y} {z}");
        }
    }

    #[test]
    fn orbit_pan_and_zoom_move_the_camera() {
        let mut camera = Camera::default();
        camera.orbit([0.0, -1000.0]);
        assert_eq!(camera.pitch, -1.55);
        camera.zoom(-100.0);
        assert!((camera.distance - 6.0 * (-0.1_f32).exp()).abs() < 1e-5);
        // A pan to the right moves the target to the left of the view, and the target stays in
        // the plane of the view.
        let (before, [right, _, forward]) = (camera.target, camera.axes());
        camera.pan([10.0, 0.0], 100.0);
        let moved = sub(camera.target, before);
        assert!(dot(moved, right) < 0.0 && dot(moved, forward).abs() < 1e-5, "{moved:?}");
        // The light comes from the side of the camera.
        assert!(dot(camera.light(), forward) < 0.0);
    }
}

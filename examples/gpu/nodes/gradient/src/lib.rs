use patina_gpu::{GpuImage, gpu, link, wgpu};
use patina_sdk::{Pointer, Ui, asset, node};
use scope::{LABEL, RIM, SCREEN, TRACE, panel, readout, round_rect};

/// The size of the panel, the strip that shows the gradient, and the switch for invert: the left
/// edge, the top, the width and the height.
const SIZE: [f32; 2] = [176.0, 92.0];
const STRIP: [f32; 4] = [14.0, 28.0, 148.0, 22.0];
const SWITCH: [f32; 4] = [44.0, 62.0, 30.0, 14.0];
/// The number of steps of the strip.
const STEPS: usize = 37;

/// Makes a gray gradient from left to right on the GPU, with the shaders in `assets/`. Click the
/// **INV** switch to go from white to black. The **invert** parameter keeps the switch.
#[node(free_layout)]
pub struct Gradient {
    invert: bool,
    size: [u32; 2],
    /// The pipeline of the last run, with the WGSL that made it.
    pipeline: Option<(String, wgpu::ComputePipeline)>,
}

impl Gradient {
    pub fn init() -> Self {
        Gradient { invert: false, size: [0, 0], pipeline: None }
    }

    pub fn process(&mut self, width: u32, height: u32, invert: bool) -> Result<GpuImage, String> {
        self.invert = invert;
        let gpu = gpu()?;
        let wgsl = link(&asset!("gradient.wesl"), &[("invert", invert)])?;
        let pipeline = match &mut self.pipeline {
            Some((w, p)) if *w == wgsl => p,
            slot => {
                let module = gpu.shader("gradient.wesl", &wgsl)?;
                let p = gpu.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("gradient"),
                    layout: None,
                    module: &module,
                    entry_point: Some("main"),
                    compilation_options: Default::default(),
                    cache: None,
                });
                &mut slot.insert((wgsl, p)).1
            }
        };
        let image = GpuImage::new(gpu, width, height);
        self.size = [image.width(), image.height()];
        let view = image.texture.create_view(&Default::default());
        let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) }],
        });
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups(image.width().div_ceil(8), image.height().div_ceil(8), 1);
        }
        gpu.queue.submit([encoder.finish()]);
        Ok(image)
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let [w, h] = self.size;
        let flip = {
            let mut c = ui.canvas(&format!("Gradient {w} x {h}"), SIZE);
            panel(&mut c, SIZE, "GRADIENT");
            if w > 0 && h > 0 {
                readout(&mut c, SIZE[0], &format!("{w} x {h}"));
            }
            let [x, y, sw, sh] = STRIP;
            round_rect(&mut c, [x - 3.0, y - 3.0], [sw + 6.0, sh + 6.0], 4.0, SCREEN);
            let step = sw / STEPS as f32;
            for i in 0..STEPS {
                let gray = (shade(i as f32 / (STEPS - 1) as f32, self.invert) * 255.0).round() as u8;
                c.rect([x + step * i as f32, y], [step, sh], [gray; 3], true);
            }
            // A slide switch: the knob is at the right and lit when the gradient is inverted.
            let [sx, sy, sw, sh] = SWITCH;
            c.text([14.0, sy + 11.0], LABEL, "INV");
            round_rect(&mut c, [sx, sy], [sw, sh], sh / 2.0, SCREEN);
            let knob = if self.invert { sx + sw - sh / 2.0 } else { sx + sh / 2.0 };
            c.circle([knob, sy + sh / 2.0], sh / 2.0 - 2.0, if self.invert { TRACE } else { RIM }, true);
            c.pointer().iter().any(|p| matches!(p, Pointer::Down(at) if on_switch(*at)))
        };
        if flip {
            ui.set_param("invert", !self.invert);
        }
        ui.pin("width", [0.0, 32.0]);
        ui.pin("height", [0.0, 50.0]);
        ui.pin("invert", [0.0, SWITCH[1] + SWITCH[3] / 2.0]);
        ui.pin("out", [SIZE[0], STRIP[1] + STRIP[3] / 2.0]);
    }
}

/// The gray at `t` from 0 to 1, with the same curve as `assets/shade.wesl`.
fn shade(t: f32, invert: bool) -> f32 {
    if invert { 1.0 - t * t } else { t * t }
}

/// True when the point is on the switch or on its label.
fn on_switch([x, y]: [f32; 2]) -> bool {
    let [sx, sy, sw, sh] = SWITCH;
    (10.0..=sx + sw + 4.0).contains(&x) && (sy - 4.0..=sy + sh + 4.0).contains(&y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_click_on_the_switch_inverts_the_gradient() {
        assert_eq!([shade(0.5, false), shade(0.5, true)], [0.25, 0.75]);
        assert!(on_switch([20.0, 69.0]) && on_switch([70.0, 65.0]));
        assert!(!on_switch([100.0, 69.0]) && !on_switch([50.0, 40.0]));
        let mut gradient = Gradient::init();
        gradient.size = [64, 16];
        let mut ui = Ui::new(0, "");
        gradient.ui(&mut ui);
        assert!(ui.tree().starts_with("canvas\tGradient 64 x 16\t176\t92\t"), "{}", ui.tree());
        assert!(ui.tree().contains("\npin\tinvert\t0 69"), "{}", ui.tree());
    }
}

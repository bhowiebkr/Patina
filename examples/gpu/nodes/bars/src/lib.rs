use patina_gpu::{GpuImage, gpu, link, wgpu};
use patina_sdk::{Ui, asset, node};
use scope::{LABEL, panel, readout, round_rect};

/// The size of the panel, and the top, the width and the height of a swatch.
const SIZE: [f32; 2] = [176.0, 104.0];
const TOP: f32 = 28.0;
const SWATCH: [f32; 2] = [20.0, 40.0];
/// The colors of the 7 bars at full level, in the order of the shader.
const COLORS: [[u8; 3]; 7] =
    [[255, 255, 255], [255, 255, 0], [0, 255, 255], [0, 255, 0], [255, 0, 255], [255, 0, 0], [0, 0, 255]];

/// Makes the color bars of a test signal on the GPU, at `level` from 0 to 1, with a gray ramp
/// under them. The shader is in `assets/`. The node GUI shows the bars as swatches.
#[node(free_layout)]
pub struct Bars {
    level: f64,
    size: [u32; 2],
}

impl Bars {
    pub fn init() -> Self {
        Bars { level: 0.0, size: [0, 0] }
    }

    pub fn process(&mut self, width: u32, height: u32, level: f64) -> Result<GpuImage, String> {
        self.level = level.clamp(0.0, 1.0);
        let gpu = gpu()?;
        let module = gpu.shader("bars.wesl", &link(&asset!("bars.wesl"), &[])?)?;
        let image = GpuImage::new(gpu, width, height);
        self.size = [image.width(), image.height()];
        let pipeline = gpu.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("bars"),
            layout: None,
            module: &module,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions {
                constants: &[("level", self.level)],
                ..Default::default()
            },
            cache: None,
        });
        let view = image.texture.create_view(&Default::default());
        let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) }],
        });
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups(image.width().div_ceil(8), image.height().div_ceil(8), 1);
        }
        gpu.queue.submit([encoder.finish()]);
        Ok(image)
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let [w, h] = self.size;
        {
            let mut c = ui.canvas(&format!("Bars {w} x {h}"), SIZE);
            panel(&mut c, SIZE, "BARS");
            readout(&mut c, SIZE[0], &format!("{:.0} %", self.level * 100.0));
            for (i, color) in COLORS.iter().enumerate() {
                let x = 14.0 + i as f32 * (SWATCH[0] + 1.0);
                round_rect(&mut c, [x, TOP], SWATCH, 2.0, dim(*color, self.level));
            }
            // The gray ramp under the bars, in 7 steps.
            for i in 0..7 {
                let x = 14.0 + i as f32 * (SWATCH[0] + 1.0);
                let gray = dim([255; 3], f64::from(i) / 6.0);
                c.rect([x, TOP + SWATCH[1] + 3.0], [SWATCH[0], 10.0], gray, true);
            }
            if w > 0 && h > 0 {
                c.text([14.0, SIZE[1] - 10.0], LABEL, &format!("{w} x {h}"));
            }
        }
        ui.pin("width", [0.0, 32.0]);
        ui.pin("height", [0.0, 50.0]);
        ui.pin("level", [0.0, 70.0]);
        ui.pin("out", [SIZE[0], TOP + SWATCH[1] / 2.0]);
    }
}

/// The color at `level` from 0 to 1.
fn dim(color: [u8; 3], level: f64) -> [u8; 3] {
    color.map(|v| (f64::from(v) * level.clamp(0.0, 1.0)).round() as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_swatches_follow_the_level() {
        assert_eq!(dim([255, 255, 0], 0.75), [191, 191, 0]);
        assert_eq!([dim([255; 3], 2.0), dim([255; 3], -1.0)], [[255; 3], [0; 3]]);
        let mut bars = Bars::init();
        (bars.level, bars.size) = (0.75, [128, 72]);
        let mut ui = Ui::new(0, "");
        bars.ui(&mut ui);
        assert!(ui.tree().starts_with("canvas\tBars 128 x 72\t176\t104\t"), "{}", ui.tree());
    }
}

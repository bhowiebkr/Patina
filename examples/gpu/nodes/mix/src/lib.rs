use patina_gpu::{GpuImage, gpu, link, wgpu};
use patina_sdk::{Canvas, Pointer, Ui, asset, node};
use scope::{LABEL, RIM, SCREEN, TRACE, panel, readout, round_rect};

/// The size of the panel, and the left end, the length and the height of the fader track.
const SIZE: [f32; 2] = [180.0, 96.0];
const TRACK: [f32; 3] = [32.0, 116.0, 54.0];
/// The cap of the fader.
const CAP: [u8; 3] = [70, 82, 96];

/// Mixes the image `b` into the image `a` on the GPU, as the crossfader of a video mixer. The
/// result has the size of `a`. Drag the fader from **A** to **B**. The **amount** parameter keeps
/// the place of the fader.
#[node(free_layout)]
pub struct Mix {
    amount: f64,
    drag: bool,
    /// The pipeline of the last run, with the WGSL and the amount that made it.
    pipeline: Option<(String, f64, wgpu::ComputePipeline)>,
}

impl Mix {
    pub fn init() -> Self {
        Mix { amount: 0.0, drag: false, pipeline: None }
    }

    pub fn process(&mut self, a: &GpuImage, b: &GpuImage, amount: f64) -> Result<GpuImage, String> {
        self.amount = amount.clamp(0.0, 1.0);
        let gpu = gpu()?;
        let wgsl = link(&asset!("mix.wesl"), &[])?;
        // A new amount needs a new pipeline, because the GPU fixes an override constant in the pipeline.
        let pipeline = match &mut self.pipeline {
            Some((w, m, p)) if *w == wgsl && *m == self.amount => p,
            slot => {
                let module = gpu.shader("mix.wesl", &wgsl)?;
                let p = gpu.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("mix"),
                    layout: None,
                    module: &module,
                    entry_point: Some("main"),
                    compilation_options: wgpu::PipelineCompilationOptions {
                        constants: &[("amount", self.amount)],
                        ..Default::default()
                    },
                    cache: None,
                });
                &mut slot.insert((wgsl, self.amount, p)).2
            }
        };
        let image = GpuImage::new(gpu, a.width(), a.height());
        let views = [a, b, &image].map(|i| i.texture.create_view(&Default::default()));
        let entries = [0, 1, 2]
            .map(|i| wgpu::BindGroupEntry { binding: i as u32, resource: wgpu::BindingResource::TextureView(&views[i]) });
        let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &entries,
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
        let before = self.amount;
        {
            let mut c = ui.canvas(&format!("Mix {:.2}", self.amount), SIZE);
            c.grab();
            for p in c.pointer() {
                match *p {
                    Pointer::Down([x, _]) => (self.drag, self.amount) = (true, place(x)),
                    Pointer::Move([x, _]) if self.drag => self.amount = place(x),
                    Pointer::Up(_) => self.drag = false,
                    _ => {}
                }
            }
            draw(&mut c, self.amount);
        }
        if self.amount != before {
            ui.set_param("amount", self.amount);
        }
        ui.pin("a", [0.0, 34.0]);
        ui.pin("b", [0.0, 56.0]);
        ui.pin("amount", [0.0, 78.0]);
        ui.pin("out", [SIZE[0], TRACK[2]]);
    }
}

/// The amount for the pointer at `x`, in steps of 0.01. The cap follows the pointer.
fn place(x: f32) -> f64 {
    (f64::from((x - TRACK[0]) / TRACK[1]) * 100.0).round().clamp(0.0, 100.0) / 100.0
}

fn draw(c: &mut Canvas, amount: f64) {
    let [left, length, y] = TRACK;
    panel(c, SIZE, "MIX");
    readout(c, SIZE[0], &format!("{amount:.2}"));
    c.text([left - 18.0, y + 4.0], LABEL, "A");
    c.text([left + length + 11.0, y + 4.0], LABEL, "B");
    for i in 0..=4 {
        let x = left + length * i as f32 / 4.0;
        c.line([x, y + 14.0], [x, y + if i % 2 == 0 { 21.0 } else { 18.0 }], RIM);
    }
    // The slot of the fader. The part from A to the cap is lit.
    let cap = left + length * amount as f32;
    round_rect(c, [left - 2.0, y - 3.0], [length + 4.0, 6.0], 3.0, SCREEN);
    c.rect([left, y - 1.0], [cap - left, 2.0], TRACE, true);
    round_rect(c, [cap - 7.0, y - 12.0], [14.0, 24.0], 3.0, CAP);
    c.line([cap - 5.0, y], [cap + 5.0, y], TRACE);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fader_follows_the_pointer() {
        assert_eq!([place(TRACK[0]), place(TRACK[0] + 29.0), place(500.0), place(0.0)], [0.0, 0.25, 1.0, 0.0]);
        let mut mix = Mix::init();
        let mut ui = Ui::new(0, "0 down 61 54\n0 move 90 54\n0 up 90 54\n0 move 148 54");
        mix.ui(&mut ui);
        assert_eq!((mix.amount, mix.drag), (0.5, false), "a move after the release does nothing");
        let mut ui = Ui::new(0, "");
        mix.ui(&mut ui);
        assert!(ui.tree().starts_with("canvas\tMix 0.50\t180\t96\t"), "{}", ui.tree());
    }
}

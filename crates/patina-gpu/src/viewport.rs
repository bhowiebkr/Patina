//! The GPU surface of a viewport (spec 15.8). The surface is on the editor window, under the
//! transparent web view.

use patina_sdk::Viewport;

use crate::{Gpu, block_on, gpu};

/// The background of the editor page (GUI guide), which shows around the area of the viewport.
const PAGE: wgpu::Color = wgpu::Color { r: 0x14 as f64 / 255.0, g: 0x14 as f64 / 255.0, b: 0x14 as f64 / 255.0, a: 1.0 };

/// The surface of a viewport. Keep it in the state of the view. A window takes only one surface,
/// and the surface goes with the state.
#[derive(Default)]
pub struct ViewportSurface(Option<Target>);

struct Target {
    window: u64,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    configured: bool,
    /// The texture of the area. Its copy goes into the window at each frame.
    area: Option<wgpu::Texture>,
}

impl ViewportSurface {
    /// Draws one frame of `viewport`. The area clears to `clear`, and `draw` records into a pass on
    /// the area, with the texture format. Before the editor places the viewport, nothing occurs.
    pub fn draw(&mut self, viewport: &Viewport, clear: wgpu::Color, draw: impl FnOnce(&mut wgpu::RenderPass<'_>, wgpu::TextureFormat)) -> Result<(), String> {
        let ([ww, wh], [x, y, w, h]) = (viewport.window_size, viewport.rect);
        // The area can go past the window while the window changes its size.
        let (w, h) = (w.min(ww.saturating_sub(x)), h.min(wh.saturating_sub(y)));
        if viewport.window == 0 || w == 0 || h == 0 {
            return Ok(());
        }
        let gpu = gpu()?;
        if self.0.as_ref().is_some_and(|t| t.window != viewport.window) {
            // The old surface goes first, because a window takes only one surface.
            self.0 = None;
        }
        let target = match &mut self.0 {
            Some(target) => target,
            None => self.0.insert(Target::new(gpu, viewport.window)?),
        };
        if [target.config.width, target.config.height] != [ww, wh] {
            (target.config.width, target.config.height, target.configured) = (ww, wh, false);
        }
        if !target.configured {
            let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
            target.surface.configure(&gpu.device, &target.config);
            // A busy GPU can fail the wait for idle of the configure. The next frame tries again.
            if let Some(error) = block_on(scope.pop()) {
                return Err(format!("the viewport surface: {error}"));
            }
            target.configured = true;
        }
        let frame = match target.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                target.configured = false;
                return Ok(());
            }
            _ => return Ok(()),
        };
        let format = target.config.format;
        let area = match &target.area {
            Some(area) if area.width() == w && area.height() == h => area,
            _ => target.area.insert(gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("viewport area"),
                size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            })),
        };
        let mut encoder = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        let area_view = area.create_view(&wgpu::TextureViewDescriptor::default());
        draw(&mut pass(&mut encoder, &area_view, clear), format);
        drop(pass(&mut encoder, &frame.texture.create_view(&wgpu::TextureViewDescriptor::default()), PAGE));
        let to = wgpu::TexelCopyTextureInfo { texture: &frame.texture, mip_level: 0, origin: wgpu::Origin3d { x, y, z: 0 }, aspect: wgpu::TextureAspect::All };
        encoder.copy_texture_to_texture(area.as_image_copy(), to, wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 });
        gpu.queue.submit([encoder.finish()]);
        gpu.queue.present(frame);
        Ok(())
    }
}

/// A render pass that clears `view` to `clear`.
fn pass<'a>(encoder: &'a mut wgpu::CommandEncoder, view: &wgpu::TextureView, clear: wgpu::Color) -> wgpu::RenderPass<'a> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("viewport"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations { load: wgpu::LoadOp::Clear(clear), store: wgpu::StoreOp::Store },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    })
}

impl Target {
    fn new(gpu: &Gpu, window: u64) -> Result<Target, String> {
        let surface = surface(gpu, window)?;
        let caps = surface.get_capabilities(&gpu.adapter);
        if !caps.usages.contains(wgpu::TextureUsages::COPY_DST) {
            return Err("The window surface does not take a copy.".to_owned());
        }
        // A format without sRGB, so that the colors match the colors of the page.
        let format = caps.formats.iter().copied().find(|f| !f.is_srgb()).or(caps.formats.first().copied()).ok_or("The window has no surface format.")?;
        let alpha_mode = match caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::Opaque) {
            true => wgpu::CompositeAlphaMode::Opaque,
            false => *caps.alpha_modes.first().ok_or("The window has no alpha mode.")?,
        };
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST,
            format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: 1,
            height: 1,
            present_mode: present_mode(&caps.present_modes),
            desired_maximum_frame_latency: 1,
            alpha_mode,
            view_formats: Vec::new(),
        };
        Ok(Target { window, surface, config, configured: false, area: None })
    }
}

/// Mailbox does not wait for the display, so the `ui` call does not stop the engine thread.
fn present_mode(modes: &[wgpu::PresentMode]) -> wgpu::PresentMode {
    match modes.contains(&wgpu::PresentMode::Mailbox) {
        true => wgpu::PresentMode::Mailbox,
        false => wgpu::PresentMode::Fifo,
    }
}

#[cfg(windows)]
fn surface(gpu: &Gpu, window: u64) -> Result<wgpu::Surface<'static>, String> {
    use wgpu::rwh::{RawDisplayHandle, RawWindowHandle, Win32WindowHandle, WindowsDisplayHandle};
    let hwnd = isize::try_from(window).ok().and_then(std::num::NonZeroIsize::new).ok_or("The window handle is not valid.")?;
    // wgpu 30 refuses a surface without a display handle.
    let target = wgpu::SurfaceTargetUnsafe::RawHandle {
        raw_display_handle: Some(RawDisplayHandle::Windows(WindowsDisplayHandle::new())),
        raw_window_handle: RawWindowHandle::Win32(Win32WindowHandle::new(hwnd)),
    };
    // SAFETY: the editor sends the handle of one of its windows. The main window lives until the
    // process stops. The editor drops the view, and with it this surface, before it destroys a
    // float window (patina-editor floats.rs).
    unsafe { gpu.instance.create_surface_unsafe(target) }.map_err(|e| format!("Patina cannot make the viewport surface: {e}"))
}

#[cfg(not(windows))]
fn surface(_: &Gpu, _: u64) -> Result<wgpu::Surface<'static>, String> {
    // shortcut: a viewport surface only on Windows (13.5), add the X11 and Wayland handles when the editor needs them.
    Err("A viewport needs Windows in this version.".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_viewport_uses_mailbox_and_waits_for_a_place() {
        assert_eq!(present_mode(&[wgpu::PresentMode::Fifo, wgpu::PresentMode::Mailbox]), wgpu::PresentMode::Mailbox);
        assert_eq!(present_mode(&[wgpu::PresentMode::Fifo]), wgpu::PresentMode::Fifo);
        let mut surface = ViewportSurface::default();
        let mut placed = Viewport { window: 7, window_size: [800, 600], rect: [900, 700, 10, 10], ..Viewport::default() };
        for viewport in [Viewport::default(), placed.clone()] {
            assert_eq!(surface.draw(&viewport, wgpu::Color::BLACK, |_, _| panic!("no draw")), Ok(()));
        }
        placed.rect = [0, 0, 0, 10];
        assert_eq!(surface.draw(&placed, wgpu::Color::BLACK, |_, _| panic!("no draw")), Ok(()));
    }
}

//! The render thread. It owns the GPU context, one surface for each window with a native
//! viewport, and the viewports. One compute pass for each frame feeds all viewports.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::WebviewWindow;

use crate::gpu::{self, Gpu, CAMERA_BYTES, COPY_FORMAT};
use crate::stats::{Frame, Stats};

const FRAME: Duration = Duration::from_micros(16_667);
/// A loop faster than this had no vsync wait, for example on an occluded window.
const NO_VSYNC: Duration = Duration::from_millis(4);
const CLEAR: wgpu::Color = wgpu::Color { r: 0.0125, g: 0.0125, b: 0.0125, a: 1.0 };
/// The bytes before the pixels of a channel frame: frame, input seq, width, height, row stride, 0.
pub const CHANNEL_HEADER: usize = 24;
const MAP_WAIT: u8 = 0;
const MAP_OK: u8 = 1;
const MAP_FAILED: u8 = 2;

/// How the frames of a viewport reach the page.
pub enum Sink {
    /// The render thread draws into the window surface under the transparent web view.
    Native,
    /// The pixels go to the page through a Tauri channel. The next frame waits for the `Ack` of
    /// the page, so frames do not queue up in the IPC.
    Channel { channel: Channel<InvokeResponseBody>, busy: bool },
    /// The pixels go into WebView2 shared memory, which the page polls.
    #[cfg(windows)]
    Shared(crate::shared::Slots),
}

pub enum Cmd {
    Viewport { id: u32, window: WebviewWindow, sink: Sink },
    Remove { id: u32 },
    /// The viewport rectangle in physical pixels of the window client area. None hides it.
    Rect { id: u32, rect: Option<[u32; 4]> },
    Input { id: u32, dx: f32, dy: f32, wheel: f32, key: bool, t_ms: f64 },
    Resize { label: String, width: u32, height: u32 },
    /// Drops the viewports and the surface of a window, then closes the window.
    CloseWindow { window: WebviewWindow },
    /// The window is gone: drop its viewports and its surface.
    WindowGone { label: String },
    LoseDevice,
    /// The page has the last channel frame of a viewport.
    Ack { id: u32 },
}

struct Camera {
    yaw: f32,
    pitch: f32,
    dist: f32,
}

impl Camera {
    fn new(id: u32) -> Self {
        Self { yaw: 0.6 + id as f32 * 1.3, pitch: 0.55, dist: 170.0 }
    }

    fn bytes(&self, aspect: f32) -> [u8; CAMERA_BYTES as usize] {
        let mut out = [0u8; CAMERA_BYTES as usize];
        for (i, v) in [self.yaw, self.pitch, self.dist, aspect].iter().enumerate() {
            out[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }
        out
    }
}

/// The GPU objects of one viewport. They are made again after a device loss.
struct ViewGpu {
    camera: wgpu::Buffer,
    group: wgpu::BindGroup,
    target: Option<CopyTarget>,
}

/// The offscreen target and the readback ring of a copy viewport.
struct CopyTarget {
    width: u32,
    height: u32,
    stride: u32,
    color: wgpu::Texture,
    view: wgpu::TextureView,
    depth: wgpu::TextureView,
    ring: [Readback; 2],
    next: usize,
    copied: bool,
}

struct Readback {
    buffer: wgpu::Buffer,
    map: Arc<AtomicU8>,
    /// The frame, the input seq and the submission of the image in the buffer, while it maps.
    pending: Option<(u32, u32, wgpu::SubmissionIndex)>,
}

struct Viewport {
    id: u32,
    window: WebviewWindow,
    sink: Sink,
    rect: Option<[u32; 4]>,
    camera: Camera,
    gpu: Option<ViewGpu>,
    frame: u32,
    /// The count of input events that this viewport received.
    seq: u32,
    /// The times of the input events since the last frame, for the latency of the native path.
    inputs: Vec<f64>,
}

impl Viewport {
    fn native(&self) -> bool {
        matches!(self.sink, Sink::Native)
    }

    fn free(self) {
        #[cfg(windows)]
        if let Sink::Shared(mut slots) = self.sink {
            slots.free(&self.window);
        }
    }
}

struct WindowSurface {
    /// The first field drops first. A surface texture that drops after its surface panics in
    /// wgpu, for example when a window closes during the vsync wait.
    current: Option<wgpu::SurfaceTexture>,
    window: WebviewWindow,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    depth: Option<wgpu::TextureView>,
    configured: bool,
}

pub struct Renderer {
    instance: wgpu::Instance,
    low_power: bool,
    gpu: Gpu,
    windows: Vec<WindowSurface>,
    views: Vec<Viewport>,
    stats: Arc<Mutex<Stats>>,
    start: Instant,
    lost_at: Option<Instant>,
    /// The bench asked for a device loss. frame() does it while the surface textures are held.
    lose_pending: bool,
    waits: Vec<f64>,
}

/// The wall clock in ms since the Unix epoch. The page sends its input times on this clock.
pub fn now_ms() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64() * 1000.0)
}

/// Starts the render thread. It stops when all senders of `cmds` are gone.
pub fn spawn(cmds: Receiver<Cmd>, stats: Arc<Mutex<Stats>>, low_power: bool) -> Result<(), String> {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    if cfg!(windows) {
        // On the Intel UHD 770, the Vulkan swapchain often did not show under the web view (STATUS.md).
        desc.backends = wgpu::Backends::DX12;
    }
    let instance = wgpu::Instance::new(desc.with_env());
    let gpu = Gpu::new(&instance, low_power)?;
    stats.lock().unwrap().adapter.clone_from(&gpu.adapter);
    let mut renderer = Renderer {
        instance,
        low_power,
        gpu,
        windows: Vec::new(),
        views: Vec::new(),
        stats,
        start: Instant::now(),
        lost_at: None,
        lose_pending: false,
        waits: Vec::with_capacity(8),
    };
    std::thread::Builder::new()
        .name("render".into())
        .spawn(move || renderer.run(&cmds))
        .map_err(|e| e.to_string())?;
    Ok(())
}

impl Renderer {
    fn run(&mut self, cmds: &Receiver<Cmd>) {
        let mut deadline = Instant::now();
        loop {
            if !self.views.iter().any(|v| v.rect.is_some()) {
                match cmds.recv_timeout(Duration::from_millis(50)) {
                    Ok(cmd) => self.apply(cmd),
                    Err(RecvTimeoutError::Timeout) => continue,
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            }
            while let Ok(cmd) = cmds.try_recv() {
                self.apply(cmd);
            }
            if self.gpu.is_lost() && !self.recover() {
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }
            let begin = Instant::now();
            let mut frame = Frame::default();
            self.frame(cmds, &mut frame);
            self.stats.lock().unwrap().frame_done(begin, &frame);
            let work = begin.elapsed();
            deadline += FRAME;
            let now = Instant::now();
            if deadline < now {
                deadline = now;
            }
            // Fifo presentation waits for the vsync. Without a present, or without a wait, sleep.
            if frame.presents == 0 || work < NO_VSYNC {
                std::thread::sleep(deadline - now);
            } else {
                deadline = now;
            }
        }
    }

    fn apply(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::Viewport { id, window, sink } => {
                self.remove(|v| v.id == id);
                self.views.push(Viewport {
                    id,
                    window,
                    sink,
                    rect: None,
                    camera: Camera::new(id),
                    gpu: None,
                    frame: 0,
                    seq: 0,
                    inputs: Vec::with_capacity(64),
                });
            }
            Cmd::Remove { id } => self.remove(|v| v.id == id),
            Cmd::Rect { id, rect } => {
                if let Some(v) = self.views.iter_mut().find(|v| v.id == id) {
                    v.rect = rect.filter(|r| r[2] > 0 && r[3] > 0);
                }
            }
            Cmd::Input { id, dx, dy, wheel, key, t_ms } => {
                let Some(v) = self.views.iter_mut().find(|v| v.id == id) else { return };
                let c = &mut v.camera;
                c.yaw += dx * 0.01;
                c.pitch = (c.pitch + dy * 0.01).clamp(-1.4, 1.4);
                c.dist = (c.dist * 1.1f32.powf(wheel / 100.0)).clamp(20.0, 800.0);
                if key {
                    *c = Camera::new(id);
                    self.stats.lock().unwrap().keys += 1;
                }
                v.seq = v.seq.wrapping_add(1);
                if v.inputs.len() < v.inputs.capacity() {
                    v.inputs.push(t_ms);
                }
            }
            Cmd::Resize { label, width, height } => {
                if let Some(w) = self.windows.iter_mut().find(|w| w.window.label() == label) {
                    w.config.width = width.max(1);
                    w.config.height = height.max(1);
                    w.configured = false;
                }
            }
            Cmd::CloseWindow { window } => {
                self.drop_window(window.label());
                if let Err(e) = window.destroy() {
                    eprintln!("viewport: cannot close {}: {e}", window.label());
                }
            }
            Cmd::WindowGone { label } => self.drop_window(&label),
            Cmd::Ack { id } => {
                if let Some(Sink::Channel { busy, .. }) = self.views.iter_mut().find(|v| v.id == id).map(|v| &mut v.sink) {
                    *busy = false;
                }
            }
            Cmd::LoseDevice => self.lose_pending = true,
        }
    }

    fn remove(&mut self, matches: impl Fn(&Viewport) -> bool) {
        while let Some(i) = self.views.iter().position(&matches) {
            self.views.swap_remove(i).free();
        }
    }

    fn drop_window(&mut self, label: &str) {
        self.remove(|v| v.window.label() == label);
        self.windows.retain(|w| w.window.label() != label);
    }

    /// Releases the held surface textures of a lost device. Else the surfaces cannot drop or
    /// configure: wgpu 30.0.2 discards a surface texture only on a valid device.
    fn release_textures(&mut self) {
        for ws in &mut self.windows {
            let Some(texture) = ws.current.take() else { continue };
            // shortcut: a drop in an unwind releases the texture (gfx-rs/wgpu 8243); remove it when wgpu does.
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                let _texture = texture;
                std::panic::resume_unwind(Box::new(()));
            }));
        }
    }

    /// Makes a new device after a loss, and the GPU objects again. Returns false on failure.
    fn recover(&mut self) -> bool {
        let started = *self.lost_at.get_or_insert_with(Instant::now);
        match Gpu::new(&self.instance, self.low_power) {
            Ok(gpu) => {
                // DX12 cannot configure an old surface on a new device. frame() makes new surfaces.
                self.windows.clear();
                self.gpu = gpu;
                for v in &mut self.views {
                    v.gpu = None;
                }
                let mut stats = self.stats.lock().unwrap();
                stats.device_lost += 1;
                stats.recovering_since = Some(started);
                true
            }
            Err(e) => {
                eprintln!("viewport: cannot make a new GPU device: {e}");
                false
            }
        }
    }

    /// Renders one frame of all visible viewports.
    #[allow(clippy::too_many_lines)]
    fn frame(&mut self, cmds: &Receiver<Cmd>, frame: &mut Frame) {
        for v in self.views.iter().filter(|v| v.native() && v.rect.is_some()) {
            if !self.windows.iter().any(|w| w.window.label() == v.window.label())
                && let Some(ws) = make_surface(&self.instance, &self.gpu.handle, &v.window)
            {
                self.stats.lock().unwrap().surface_format = format!("{:?}", ws.config.format);
                self.windows.push(ws);
            }
        }
        // Get the surface textures first: Fifo blocks here until a vsync.
        let gpu = &mut self.gpu;
        for ws in &mut self.windows {
            let label = ws.window.label();
            if !self.views.iter().any(|v| v.native() && v.rect.is_some() && v.window.label() == label) {
                continue;
            }
            if !ws.configured {
                gpu.scene.prepare(&gpu.device, ws.config.format);
                ws.surface.configure(&gpu.device, &ws.config);
                ws.depth = Some(gpu::depth_texture(&gpu.device, ws.config.width, ws.config.height));
                ws.configured = true;
                self.stats.lock().unwrap().surfaces_configured += 1;
            }
            match ws.surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => {
                    ws.current = Some(t);
                }
                wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => ws.configured = false,
                _ => {}
            }
        }
        // Take the input that came during the wait, so that this frame shows it.
        while let Ok(cmd) = cmds.try_recv() {
            self.apply(cmd);
        }
        if self.lose_pending {
            self.lose_pending = false;
            self.lost_at = Some(Instant::now());
            self.gpu.device.destroy();
            // The callback can come later. Do not use the destroyed device until then.
            self.gpu.lost.store(true, Ordering::SeqCst);
        }
        if self.gpu.is_lost() {
            self.release_textures();
            return;
        }

        let gpu = &mut self.gpu;
        let time = self.start.elapsed().as_secs_f32();
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        gpu.scene.animate(&gpu.queue, &mut encoder, time);
        frame.upload_bytes += 16;
        for v in self.views.iter_mut().filter(|v| v.rect.is_some()) {
            let view_gpu = v.gpu.get_or_insert_with(|| {
                let camera = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("camera"),
                    size: CAMERA_BYTES,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                ViewGpu { group: gpu.scene.view_group(&gpu.device, &camera), camera, target: None }
            });
            let [_, _, w, h] = v.rect.unwrap_or_default();
            gpu.queue.write_buffer(&view_gpu.camera, 0, &v.camera.bytes(w as f32 / h.max(1) as f32));
            frame.upload_bytes += CAMERA_BYTES;
        }

        for ws in &mut self.windows {
            let Some(texture) = &ws.current else { continue };
            let (tw, th) = (texture.texture.width(), texture.texture.height());
            let target = texture.texture.create_view(&Default::default());
            let mut pass = begin_pass(&mut encoder, &target, ws.depth.as_ref());
            let label = ws.window.label();
            for v in self.views.iter().filter(|v| v.native() && v.window.label() == label) {
                let (Some(rect), Some(view_gpu)) = (v.rect, &v.gpu) else { continue };
                let Some([x, y, w, h]) = clamp_rect(rect, tw, th) else { continue };
                pass.set_viewport(x as f32, y as f32, w as f32, h as f32, 0.0, 1.0);
                pass.set_scissor_rect(x, y, w, h);
                gpu.scene.draw(&mut pass, texture.texture.format(), &view_gpu.group);
            }
        }

        gpu.scene.prepare(&gpu.device, COPY_FORMAT);
        for v in self.views.iter_mut().filter(|v| !v.native()) {
            let (Some([_, _, w, h]), Some(view_gpu)) = (v.rect, v.gpu.as_mut()) else { continue };
            if !matches!(&view_gpu.target, Some(t) if t.width == w && t.height == h) {
                view_gpu.target = Some(CopyTarget::new(&gpu.device, w, h));
            }
            let Some(target) = view_gpu.target.as_mut() else { continue };
            {
                let mut pass = begin_pass(&mut encoder, &target.view, Some(&target.depth));
                gpu.scene.draw(&mut pass, COPY_FORMAT, &view_gpu.group);
            }
            let rb = &target.ring[target.next];
            // A buffer that still waits for the page drops this frame.
            target.copied = rb.pending.is_none();
            if target.copied {
                encoder.copy_texture_to_buffer(
                    target.color.as_image_copy(),
                    wgpu::TexelCopyBufferInfo {
                        buffer: &rb.buffer,
                        layout: wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(target.stride),
                            rows_per_image: Some(h),
                        },
                    },
                    wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                );
            } else {
                frame.dropped += 1;
            }
        }

        let submit = gpu.queue.submit([encoder.finish()]);
        // shortcut: a real device loss after the check above keeps the texture acquired; the Vulkan surface drop then panics.
        for ws in &mut self.windows {
            if let Some(t) = ws.current.take() {
                gpu.queue.present(t);
                frame.presents += 1;
            }
        }
        let present_ms = now_ms();

        self.waits.clear();
        for v in self.views.iter_mut().filter(|v| v.rect.is_some()) {
            v.frame = v.frame.wrapping_add(1);
            frame.view_frames += 1;
            if v.native() {
                continue;
            }
            v.inputs.clear();
            let Some(target) = v.gpu.as_mut().and_then(|g| g.target.as_mut()) else { continue };
            if target.copied {
                let rb = &mut target.ring[target.next];
                rb.map.store(MAP_WAIT, Ordering::SeqCst);
                let flag = rb.map.clone();
                rb.buffer.map_async(wgpu::MapMode::Read, .., move |r| {
                    flag.store(if r.is_ok() { MAP_OK } else { MAP_FAILED }, Ordering::SeqCst);
                });
                rb.pending = Some((v.frame, v.seq, submit.clone()));
                target.next = 1 - target.next;
            }
            // Read the image of the previous frame: the GPU is most likely done with it.
            let prev = &mut target.ring[target.next];
            let Some((image_frame, seq, index)) = prev.pending.clone() else { continue };
            let wait = Instant::now();
            let _ = gpu.device.poll(wgpu::PollType::Wait { submission_index: Some(index), timeout: Some(Duration::from_secs(1)) });
            if self.waits.len() < self.waits.capacity() {
                self.waits.push(wait.elapsed().as_secs_f64() * 1000.0);
            }
            match prev.map.load(Ordering::SeqCst) {
                MAP_OK => {}
                MAP_FAILED => {
                    prev.pending = None;
                    frame.dropped += 1;
                    continue;
                }
                _ => {
                    frame.dropped += 1;
                    continue;
                }
            }
            let header = [image_frame, seq, target.width, target.height, target.stride, 0];
            let sent = match prev.buffer.get_mapped_range(..) {
                Ok(data) => send(&mut v.sink, &v.window, v.id, &header, &data),
                Err(_) => None,
            };
            prev.buffer.unmap();
            prev.pending = None;
            match sent {
                Some(bytes) => {
                    frame.copies += 1;
                    frame.copy_bytes += bytes as u64;
                }
                None => frame.dropped += 1,
            }
        }

        let mut stats = self.stats.lock().unwrap();
        for v in self.views.iter_mut().filter(|v| v.native()) {
            for t in v.inputs.drain(..) {
                stats.latency_ms.push(present_ms - t);
            }
        }
        for w in &self.waits {
            stats.readback_ms.push(*w);
        }
        if frame.presents + frame.copies > 0
            && let Some(since) = stats.recovering_since.take()
        {
            stats.recovery_ms.push(since.elapsed().as_secs_f64() * 1000.0);
            self.lost_at = None;
        }
    }
}

fn make_surface(instance: &wgpu::Instance, adapter: &wgpu::Adapter, window: &WebviewWindow) -> Option<WindowSurface> {
    let size = window.inner_size().ok()?;
    let surface = instance
        .create_surface(window.clone())
        .map_err(|e| eprintln!("viewport: cannot make a surface for {}: {e}", window.label()))
        .ok()?;
    let caps = surface.get_capabilities(adapter);
    // A format without sRGB, so that the colors match the copy path.
    let format = caps.formats.iter().copied().find(|f| !f.is_srgb()).or(caps.formats.first().copied())?;
    let alpha_mode = if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::Opaque) {
        wgpu::CompositeAlphaMode::Opaque
    } else {
        *caps.alpha_modes.first()?
    };
    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        color_space: wgpu::SurfaceColorSpace::Auto,
        width: size.width.max(1),
        height: size.height.max(1),
        present_mode: wgpu::PresentMode::Fifo,
        desired_maximum_frame_latency: 1,
        alpha_mode,
        view_formats: Vec::new(),
    };
    Some(WindowSurface { window: window.clone(), surface, config, depth: None, configured: false, current: None })
}

/// Sends one frame to the page. Returns the bytes that the CPU copied, or None for a dropped frame.
fn send(sink: &mut Sink, window: &WebviewWindow, id: u32, header: &[u32; 6], data: &[u8]) -> Option<usize> {
    match sink {
        Sink::Native => None,
        Sink::Channel { channel, busy } => {
            let _ = (window, id);
            if *busy {
                return None;
            }
            // shortcut: the Tauri channel takes an owned Vec, so each frame allocates; the shared path does not.
            let mut bytes = Vec::with_capacity(CHANNEL_HEADER + data.len());
            for v in header {
                bytes.extend_from_slice(&v.to_le_bytes());
            }
            bytes.extend_from_slice(data);
            let len = bytes.len();
            channel.send(InvokeResponseBody::Raw(bytes)).ok()?;
            *busy = true;
            Some(len)
        }
        #[cfg(windows)]
        Sink::Shared(slots) => slots.send(window, id, header, data),
    }
}

fn begin_pass<'a>(
    encoder: &'a mut wgpu::CommandEncoder,
    target: &wgpu::TextureView,
    depth: Option<&wgpu::TextureView>,
) -> wgpu::RenderPass<'a> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("view"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations { load: wgpu::LoadOp::Clear(CLEAR), store: wgpu::StoreOp::Store },
        })],
        depth_stencil_attachment: depth.map(|view| wgpu::RenderPassDepthStencilAttachment {
            view,
            depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }),
            stencil_ops: None,
        }),
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    })
}

/// Clips a viewport rectangle to the surface. Returns None when nothing is left.
fn clamp_rect([x, y, w, h]: [u32; 4], sw: u32, sh: u32) -> Option<[u32; 4]> {
    let x1 = x.saturating_add(w).min(sw);
    let y1 = y.saturating_add(h).min(sh);
    (x < x1 && y < y1).then(|| [x, y, x1 - x, y1 - y])
}

impl CopyTarget {
    fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let stride = (width * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let color = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("copy target"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: COPY_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let readback = || Readback {
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("readback"),
                size: u64::from(stride) * u64::from(height),
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            map: Arc::new(AtomicU8::new(MAP_WAIT)),
            pending: None,
        };
        Self {
            width,
            height,
            stride,
            view: color.create_view(&Default::default()),
            color,
            depth: gpu::depth_texture(device, width, height),
            ring: [readback(), readback()],
            next: 0,
            copied: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_rect_clips_to_the_surface() {
        assert_eq!(clamp_rect([10, 10, 100, 50], 1920, 1080), Some([10, 10, 100, 50]));
        assert_eq!(clamp_rect([1900, 1000, 100, 100], 1920, 1080), Some([1900, 1000, 20, 80]));
        assert_eq!(clamp_rect([2000, 10, 100, 100], 1920, 1080), None);
        assert_eq!(clamp_rect([u32::MAX, 0, 5, 5], 1920, 1080), None);
    }
}

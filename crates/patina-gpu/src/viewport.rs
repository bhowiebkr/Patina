//! The GPU surface of a viewport (spec 15.8). The surface is on a child window of the editor
//! window at the area of the viewport, under the transparent web view.

use patina_sdk::Viewport;

use crate::{Gpu, block_on, gpu};

/// The surface of a viewport. Keep it in the state of the view. A window takes only one surface,
/// and the surface goes with the state.
#[derive(Default)]
pub struct ViewportSurface(Option<Target>);

struct Target {
    window: u64,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    configured: bool,
    /// The place of the window in the editor window: x, y, width and height.
    place: [u32; 4],
    /// The window of the surface. It is the last field, so it drops after the surface.
    child: child::Child,
}

impl ViewportSurface {
    /// Draws one frame of `viewport`. The area clears to `clear`, and `draw` records into a pass on
    /// the area, with the texture format. Before the editor places the viewport, nothing occurs. An
    /// empty area (a hidden panel) hides the window.
    pub fn draw(&mut self, viewport: &Viewport, clear: wgpu::Color, draw: impl FnOnce(&mut wgpu::RenderPass<'_>, wgpu::TextureFormat)) -> Result<(), String> {
        let ([ww, wh], [x, y, w, h]) = (viewport.window_size, viewport.rect);
        // The area can go past the window while the window changes its size.
        let (w, h) = (w.min(ww.saturating_sub(x)), h.min(wh.saturating_sub(y)));
        if viewport.window == 0 {
            return Ok(());
        }
        if w == 0 || h == 0 {
            if let Some(target) = &mut self.0 {
                target.place([x, y, 0, 0]);
            }
            return Ok(());
        }
        let gpu = gpu()?;
        if self.0.as_ref().is_some_and(|t| t.window != viewport.window) {
            // The old surface goes first, because a window takes only one surface.
            self.0 = None;
        }
        let target = match &mut self.0 {
            Some(target) => target,
            None => self.0.insert(Target::new(gpu, viewport.window, [x, y, w, h])?),
        };
        target.place([x, y, w, h]);
        if [target.config.width, target.config.height] != [w, h] {
            (target.config.width, target.config.height, target.configured) = (w, h, false);
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
            wgpu::CurrentSurfaceTexture::Outdated => {
                target.configured = false;
                return Ok(());
            }
            // A lost surface does not come back, so the next frame makes a new window and surface.
            wgpu::CurrentSurfaceTexture::Lost => {
                self.0 = None;
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Validation => return Err("the viewport surface has no frame".to_owned()),
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => return Ok(()),
        };
        let mut encoder = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        draw(&mut pass(&mut encoder, &view, clear), target.config.format);
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
    fn new(gpu: &Gpu, window: u64, place: [u32; 4]) -> Result<Target, String> {
        let child = child::Child::new(window, place)?;
        let surface = child.surface(gpu)?;
        let caps = surface.get_capabilities(&gpu.adapter);
        // A format without sRGB, so that the colors match the colors of the page.
        let format = caps.formats.iter().copied().find(|f| !f.is_srgb()).or(caps.formats.first().copied()).ok_or("The window has no surface format.")?;
        let alpha_mode = match caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::Opaque) {
            true => wgpu::CompositeAlphaMode::Opaque,
            false => *caps.alpha_modes.first().ok_or("The window has no alpha mode.")?,
        };
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: 1,
            height: 1,
            present_mode: present_mode(&caps.present_modes),
            desired_maximum_frame_latency: 1,
            alpha_mode,
            view_formats: Vec::new(),
        };
        Ok(Target { window, surface, config, configured: false, place, child })
    }

    /// Moves the window to `place`, only when the place changes.
    fn place(&mut self, place: [u32; 4]) {
        if self.place != place {
            self.place = place;
            self.child.place(place);
        }
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
mod child {
    //! The window of a viewport surface: a child of the editor window that a thread of this process
    //! owns. When the node process ends, Windows destroys the window, and the surface goes with it.
    //! A surface on the editor window itself stays for seconds after a kill, and the next node
    //! process cannot make its surface in that time.

    use std::ffi::c_void;
    use std::ptr::null;
    use std::sync::mpsc;
    use std::thread::JoinHandle;

    use crate::Gpu;

    const WS_CHILD: u32 = 0x4000_0000;
    const WS_VISIBLE: u32 = 0x1000_0000;
    const WS_DISABLED: u32 = 0x0800_0000;
    const WS_EX_NOPARENTNOTIFY: u32 = 0x4;
    const HWND_BOTTOM: isize = 1;
    const SWP_NOSIZE: u32 = 0x1;
    const SWP_NOMOVE: u32 = 0x2;
    const SWP_NOZORDER: u32 = 0x4;
    const SWP_NOACTIVATE: u32 = 0x10;
    const SWP_ASYNCWINDOWPOS: u32 = 0x4000;
    const WM_APP: u32 = 0x8000;
    /// The message to the window thread that sets the place of the window.
    const PLACE: u32 = WM_APP + 1;

    #[repr(C)]
    struct Msg {
        hwnd: isize,
        message: u32,
        wparam: usize,
        lparam: isize,
        time: u32,
        pt: [i32; 2],
    }

    #[repr(C)]
    struct WndClass {
        style: u32,
        proc: unsafe extern "system" fn(isize, u32, usize, isize) -> isize,
        cls_extra: i32,
        wnd_extra: i32,
        instance: isize,
        icon: isize,
        cursor: isize,
        background: isize,
        menu: *const u16,
        name: *const u16,
    }

    // SAFETY: the signatures match the Win32 headers, with isize for each handle.
    #[link(name = "user32")]
    unsafe extern "system" {
        fn RegisterClassW(class: *const WndClass) -> u16;
        fn CreateWindowExW(ex: u32, class: *const u16, name: *const u16, style: u32, x: i32, y: i32, w: i32, h: i32, parent: isize, menu: isize, instance: isize, param: *const c_void) -> isize;
        fn DefWindowProcW(hwnd: isize, message: u32, wparam: usize, lparam: isize) -> isize;
        fn GetMessageW(msg: *mut Msg, hwnd: isize, min: u32, max: u32) -> i32;
        fn DispatchMessageW(msg: *const Msg) -> isize;
        fn DestroyWindow(hwnd: isize) -> i32;
        fn PostThreadMessageW(thread: u32, message: u32, wparam: usize, lparam: isize) -> i32;
        fn SetWindowPos(hwnd: isize, after: isize, x: i32, y: i32, w: i32, h: i32, flags: u32) -> i32;
        fn GetWindowDpiAwarenessContext(hwnd: isize) -> isize;
        fn SetThreadDpiAwarenessContext(context: isize) -> isize;
    }

    // SAFETY: the signatures match the Win32 headers.
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetModuleHandleW(name: *const u16) -> isize;
        fn GetCurrentThreadId() -> u32;
    }

    pub struct Child {
        hwnd: isize,
        thread: u32,
        pump: Option<JoinHandle<()>>,
    }

    impl Child {
        /// Makes a window at `place` in the client area of `parent`, under the other children of
        /// `parent`, so under the web view.
        pub fn new(parent: u64, place: [u32; 4]) -> Result<Child, String> {
            let parent = isize::try_from(parent).ok().filter(|&p| p != 0).ok_or("The window handle is not valid.")?;
            let (send, made) = mpsc::channel();
            let pump = std::thread::Builder::new().name("patina-viewport-window".to_owned()).spawn(move || {
                let made = make(parent, place);
                let hwnd = made.as_ref().map_or(0, |&(hwnd, _)| hwnd);
                let _ = send.send(made);
                if hwnd == 0 {
                    return;
                }
                // The window shares its input with the editor window, so its thread reads the messages.
                let mut msg = Msg { hwnd: 0, message: 0, wparam: 0, lparam: 0, time: 0, pt: [0; 2] };
                // SAFETY: `msg` is a MSG that GetMessageW writes.
                while unsafe { GetMessageW(&mut msg, 0, 0, 0) } > 0 && !(msg.hwnd == 0 && msg.message == WM_APP) {
                    if msg.hwnd == 0 && msg.message == PLACE {
                        let ([x, y], [w, h]) = (unpack(msg.wparam), unpack(msg.lparam as usize));
                        // SAFETY: this thread made the window, and the place comes from `place`.
                        unsafe { SetWindowPos(hwnd, 0, x, y, w, h, SWP_NOZORDER | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS) };
                        continue;
                    }
                    // SAFETY: GetMessageW wrote `msg`.
                    unsafe { DispatchMessageW(&msg) };
                }
                // SAFETY: this thread made the window. A handle that is not valid gives an error only.
                unsafe { DestroyWindow(hwnd) };
            });
            let pump = pump.map_err(|e| format!("The viewport window cannot start: {e}"))?;
            let made = made.recv().unwrap_or_else(|_| Err("The viewport window stopped.".to_owned()));
            match made {
                Ok((hwnd, thread)) => Ok(Child { hwnd, thread, pump: Some(pump) }),
                Err(e) => {
                    let _ = pump.join();
                    Err(e)
                }
            }
        }

        /// Sets the place of the window. The window thread does the change, with its DPI awareness,
        /// so the caller does not wait for the editor.
        pub fn place(&self, [x, y, w, h]: [u32; 4]) {
            // SAFETY: PostThreadMessageW takes plain values, and the thread has a message queue.
            unsafe { PostThreadMessageW(self.thread, PLACE, pack([x, y]), pack([w, h]) as isize) };
        }

        pub fn surface(&self, gpu: &Gpu) -> Result<wgpu::Surface<'static>, String> {
            use wgpu::rwh::{RawDisplayHandle, RawWindowHandle, Win32WindowHandle, WindowsDisplayHandle};
            let hwnd = std::num::NonZeroIsize::new(self.hwnd).ok_or("The viewport window is not valid.")?;
            // wgpu 30 refuses a surface without a display handle.
            let target = wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: Some(RawDisplayHandle::Windows(WindowsDisplayHandle::new())),
                raw_window_handle: RawWindowHandle::Win32(Win32WindowHandle::new(hwnd)),
            };
            // SAFETY: the window lives until this `Child` drops, and the surface drops first (the
            // field order of `Target`). The editor drops the view before it destroys a float
            // window, the parent (patina-editor floats.rs).
            unsafe { gpu.instance.create_surface_unsafe(target) }.map_err(|e| format!("Patina cannot make the viewport surface: {e}"))
        }
    }

    impl Drop for Child {
        /// Stops the window thread, which destroys the window. The join comes before the view
        /// library unloads.
        fn drop(&mut self) {
            // SAFETY: PostThreadMessageW takes plain values. The thread has a message queue,
            // because it made the window.
            unsafe { PostThreadMessageW(self.thread, WM_APP, 0, 0) };
            if let Some(pump) = self.pump.take() {
                let _ = pump.join();
            }
        }
    }

    fn pixels(v: u32) -> i32 {
        i32::try_from(v).unwrap_or(i32::MAX)
    }

    /// Puts two values of 16 bits into one message value.
    fn pack([a, b]: [u32; 2]) -> usize {
        ((a.min(0xffff) << 16) | b.min(0xffff)) as usize
    }

    fn unpack(v: usize) -> [i32; 2] {
        [((v >> 16) & 0xffff) as i32, (v & 0xffff) as i32]
    }

    /// Makes the window in this thread, and gives its handle and the ID of this thread.
    fn make(parent: isize, [x, y, w, h]: [u32; 4]) -> Result<(isize, u32), String> {
        // A node process is not DPI aware. Without the awareness of the editor window, Windows
        // reads the place of the child in logical pixels, and the picture moves on a scaled display.
        // SAFETY: both calls take plain values. A handle that is not valid gives 0, and the thread keeps its awareness.
        unsafe { SetThreadDpiAwarenessContext(GetWindowDpiAwarenessContext(parent)) };
        let name: Vec<u16> = "PatinaViewport\0".encode_utf16().collect();
        // SAFETY: a null name gives the handle of the program file.
        let instance = unsafe { GetModuleHandleW(null()) };
        // The window draws nothing itself, so it has no background brush.
        let class = WndClass { style: 0, proc: DefWindowProcW, cls_extra: 0, wnd_extra: 0, instance, icon: 0, cursor: 0, background: 0, menu: null(), name: name.as_ptr() };
        // SAFETY: `class` and `name` live during the call. A second call fails because the class
        // exists, and CreateWindowExW finds the class.
        unsafe { RegisterClassW(&class) };
        // SAFETY: `name` ends with a zero. WS_DISABLED sends the mouse to the web view, and
        // WS_EX_NOPARENTNOTIFY keeps the editor thread out of the creation and the destruction.
        let hwnd = unsafe { CreateWindowExW(WS_EX_NOPARENTNOTIFY, name.as_ptr(), null(), WS_CHILD | WS_VISIBLE | WS_DISABLED, pixels(x), pixels(y), pixels(w), pixels(h), parent, 0, instance, null()) };
        if hwnd == 0 {
            return Err(format!("Patina cannot make the viewport window: {}", std::io::Error::last_os_error()));
        }
        // SAFETY: this thread made the window. A new child is on top, and the web view must stay on top.
        unsafe { SetWindowPos(hwnd, HWND_BOTTOM, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE) };
        // SAFETY: GetCurrentThreadId has no arguments.
        Ok((hwnd, unsafe { GetCurrentThreadId() }))
    }

    #[test]
    fn a_place_goes_through_one_message_value() {
        assert_eq!(unpack(pack([1920, 7])), [1920, 7]);
        assert_eq!(unpack(pack([70_000, 0])), [0xffff, 0]);
    }
}

#[cfg(not(windows))]
mod child {
    use crate::Gpu;

    /// A viewport window, which has no values outside Windows.
    pub enum Child {}

    impl Child {
        pub fn new(_: u64, _: [u32; 4]) -> Result<Child, String> {
            // shortcut: a viewport surface only on Windows (13.5), add the X11 and Wayland handles when the editor needs them.
            Err("A viewport needs Windows in this version.".to_owned())
        }

        pub fn place(&self, _: [u32; 4]) {
            match *self {}
        }

        pub fn surface(&self, _: &Gpu) -> Result<wgpu::Surface<'static>, String> {
            match *self {}
        }
    }
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

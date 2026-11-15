//! The copy path with WebView2 shared memory. Each copy viewport gets one buffer with three
//! slots. The render thread writes a frame into a free slot and marks it ready. The page polls
//! the slots in each animation frame, uploads the newest ready slot and marks the slots free.
//! No message goes through IPC for a frame.

use std::cell::RefCell;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::sync_channel;
use std::time::Duration;

use tauri::WebviewWindow;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    ICoreWebView2Environment12, ICoreWebView2SharedBuffer, ICoreWebView2_17,
    COREWEBVIEW2_SHARED_BUFFER_ACCESS_READ_WRITE,
};
use windows::core::{Interface, HSTRING};

/// The bytes before the pixels of a slot: state, frame, input seq, width, height, stride, 0, 0.
pub const HEADER: usize = 32;
const SLOTS: usize = 3;
const FREE: u32 = 0;
const READY: u32 = 1;

static NEXT_KEY: AtomicU32 = AtomicU32::new(1);

thread_local! {
    /// The buffers of the host, on the main thread. A COM object stays on the thread that made it.
    static BUFFERS: RefCell<Vec<(u32, ICoreWebView2SharedBuffer)>> = const { RefCell::new(Vec::new()) };
}

/// The shared buffer of one viewport, seen from the render thread.
#[derive(Default)]
pub struct Slots {
    key: u32,
    slot_len: usize,
    /// The address of the host mapping. 0 when there is no buffer.
    base: usize,
}

impl Slots {
    /// Writes a frame into a free slot. Returns the bytes copied, or None when no slot is free.
    pub fn send(&mut self, window: &WebviewWindow, vp: u32, header: &[u32; 6], data: &[u8]) -> Option<usize> {
        let slot_len = (HEADER + data.len()).next_multiple_of(256);
        if slot_len != self.slot_len {
            // shortcut: a resize makes a new buffer and waits for the main thread; keep spare room when resizes are frequent.
            self.free(window);
            self.key = NEXT_KEY.fetch_add(1, Ordering::Relaxed);
            self.slot_len = slot_len;
            match alloc(window, vp, self.key, slot_len) {
                Ok(base) => self.base = base,
                Err(e) => eprintln!("viewport: no shared buffer for viewport {vp}: {e}"),
            }
        }
        if self.base == 0 {
            return None;
        }
        for i in 0..SLOTS {
            let slot = self.base + i * self.slot_len;
            // SAFETY: the slot is inside the host mapping, which stays valid until `free` posts
            // Close. The first word is 4-byte aligned, because the mapping and slot_len are.
            let state = unsafe { &*(slot as *const AtomicU32) };
            if state.load(Ordering::Acquire) != FREE {
                continue;
            }
            // SAFETY: the page does not read a free slot, and HEADER + data fits in slot_len.
            unsafe {
                std::ptr::copy_nonoverlapping(data.as_ptr(), (slot + HEADER) as *mut u8, data.len());
                let words = slot as *mut u32;
                for (k, v) in header[..5].iter().enumerate() {
                    words.add(k + 1).write_volatile(*v);
                }
            }
            state.store(READY, Ordering::Release);
            return Some(data.len());
        }
        None
    }

    /// Releases the host side of the buffer. The page releases its side when a new key comes.
    pub fn free(&mut self, window: &WebviewWindow) {
        if self.key == 0 {
            return;
        }
        let key = std::mem::take(&mut self.key);
        self.base = 0;
        let close = move || {
            BUFFERS.with_borrow_mut(|buffers| {
                buffers.retain(|(k, buffer)| {
                    if *k == key {
                        // SAFETY: a COM call on the thread that made the buffer.
                        let _ = unsafe { buffer.Close() };
                    }
                    *k != key
                });
            });
        };
        if let Err(e) = window.run_on_main_thread(close) {
            eprintln!("viewport: cannot release a shared buffer: {e}");
        }
    }
}

/// Makes a buffer of SLOTS slots on the main thread and posts it to the page.
fn alloc(window: &WebviewWindow, vp: u32, key: u32, slot_len: usize) -> Result<usize, String> {
    let (done, result) = sync_channel::<Result<usize, String>>(1);
    window
        .with_webview(move |webview| {
            let _ = done.send(post(&webview, vp, key, slot_len).map_err(|e| e.to_string()));
        })
        .map_err(|e| e.to_string())?;
    result.recv_timeout(Duration::from_secs(2)).map_err(|e| e.to_string())?
}

fn post(webview: &tauri::webview::PlatformWebview, vp: u32, key: u32, slot_len: usize) -> windows::core::Result<usize> {
    let size = (slot_len * SLOTS) as u64;
    let info = HSTRING::from(format!(r#"{{"vp":{vp},"key":{key},"slots":{SLOTS},"slot_len":{slot_len}}}"#));
    // SAFETY: with_webview runs this on the main thread, which owns the WebView2 objects. The
    // buffer stays in BUFFERS until `free` closes it, so the mapping outlives the returned address.
    unsafe {
        let env: ICoreWebView2Environment12 = webview.environment().cast()?;
        let core: ICoreWebView2_17 = webview.controller().CoreWebView2()?.cast()?;
        let buffer = env.CreateSharedBuffer(size)?;
        let mut ptr = std::ptr::null_mut::<u8>();
        buffer.Buffer(&mut ptr)?;
        core.PostSharedBufferToScript(&buffer, COREWEBVIEW2_SHARED_BUFFER_ACCESS_READ_WRITE, &info)?;
        BUFFERS.with_borrow_mut(|buffers| buffers.push((key, buffer)));
        Ok(ptr as usize)
    }
}

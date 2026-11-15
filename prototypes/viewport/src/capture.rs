//! Bench captures on Windows. PrintWindow asks DWM for the composed client area, so the image
//! shows the native surface and the web view together, also for an off-screen window.

use std::sync::mpsc::{channel, RecvTimeoutError};
use std::time::Duration;

use tauri::WebviewWindow;
use webview2_com::{CallDevToolsProtocolMethodCompletedHandler, CoTaskMemPWSTR};
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, ReleaseDC, SelectObject, BITMAPINFO,
    BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
};
use windows::Win32::Storage::Xps::{PrintWindow, PRINT_WINDOW_FLAGS, PW_CLIENTONLY};
use windows::Win32::UI::WindowsAndMessaging::{GetClientRect, PW_RENDERFULLCONTENT};

/// A pixel with a chroma above this is part of the scene. The panels are neutral grey.
const SCENE_CHROMA: u8 = 40;

pub struct Shot {
    pub width: u32,
    pub height: u32,
    /// Top-down rows of BGRA pixels.
    pub bgra: Vec<u8>,
}

impl Shot {
    /// The part of the pixels in `rect` (physical x, y, width, height) that show the scene.
    /// It reads a grid of 32 x 32 points.
    pub fn scene_fraction(&self, [x, y, w, h]: [u32; 4]) -> f64 {
        let (mut hits, mut total) = (0u32, 0u32);
        for j in 0..32 {
            for i in 0..32 {
                let (px, py) = (x + w * (2 * i + 1) / 64, y + h * (2 * j + 1) / 64);
                if px >= self.width || py >= self.height {
                    continue;
                }
                let at = ((py * self.width + px) * 4) as usize;
                let p = &self.bgra[at..at + 3];
                let chroma = p.iter().max().unwrap_or(&0) - p.iter().min().unwrap_or(&0);
                total += 1;
                hits += u32::from(chroma > SCENE_CHROMA);
            }
        }
        if total == 0 {
            0.0
        } else {
            f64::from(hits) / f64::from(total)
        }
    }

    /// The image as a 32-bit BMP file.
    pub fn bmp(&self) -> Vec<u8> {
        let size = 54 + self.bgra.len() as u32;
        let mut out = Vec::with_capacity(size as usize);
        out.extend_from_slice(b"BM");
        for v in [size, 0, 54, 40] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.extend_from_slice(&(self.width as i32).to_le_bytes());
        out.extend_from_slice(&(-(self.height as i32)).to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&32u16.to_le_bytes());
        out.extend_from_slice(&[0; 24]);
        out.extend_from_slice(&self.bgra);
        out
    }
}

/// Captures the client area of the window.
pub fn print_window(window: &WebviewWindow) -> Result<Shot, String> {
    let hwnd = window.hwnd().map_err(|e| e.to_string())?;
    let mut rect = RECT::default();
    // SAFETY: the HWND belongs to a live window of this process. GetClientRect writes into `rect`.
    unsafe { GetClientRect(hwnd, &mut rect) }.map_err(|e| e.to_string())?;
    let (width, height) = ((rect.right - rect.left).max(0), (rect.bottom - rect.top).max(0));
    if width == 0 || height == 0 {
        return Err("the window has no client area".into());
    }
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let len = (width * height * 4) as usize;
    // SAFETY: each GDI object is released before the block ends. The DIB has `len` bytes, and
    // the copy reads them only after PrintWindow returns.
    unsafe {
        let screen = GetDC(None);
        let dc = CreateCompatibleDC(Some(screen));
        let mut bits = std::ptr::null_mut();
        let result = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0).map(|bitmap| {
            let old = SelectObject(dc, bitmap.into());
            let flags = PRINT_WINDOW_FLAGS(PW_CLIENTONLY.0 | PW_RENDERFULLCONTENT);
            let ok = PrintWindow(hwnd, dc, flags).as_bool();
            let pixels = std::slice::from_raw_parts(bits.cast::<u8>(), len).to_vec();
            SelectObject(dc, old);
            let _ = DeleteObject(bitmap.into());
            (ok, pixels)
        });
        let _ = DeleteDC(dc);
        ReleaseDC(None, screen);
        match result {
            Ok((true, bgra)) => Ok(Shot { width: width as u32, height: height as u32, bgra }),
            Ok((false, _)) => Err("PrintWindow failed".into()),
            Err(e) => Err(e.to_string()),
        }
    }
}

/// Calls a DevTools protocol method of the page and returns the JSON result.
pub fn cdp(window: &WebviewWindow, method: &str, params: &str) -> Result<String, String> {
    let (done, result) = channel::<Result<String, String>>();
    let (method, params) = (method.to_string(), params.to_string());
    window
        .with_webview(move |webview| {
            let (method, params) = (CoTaskMemPWSTR::from(method.as_str()), CoTaskMemPWSTR::from(params.as_str()));
            let reply = done.clone();
            let handler = CallDevToolsProtocolMethodCompletedHandler::create(Box::new(move |status, json| {
                let _ = reply.send(status.map(|()| json).map_err(|e| e.to_string()));
                Ok(())
            }));
            // SAFETY: with_webview runs this on the WebView2 thread. The call copies the strings,
            // which live until it returns, and keeps its own reference to the handler.
            let call = unsafe {
                webview.controller().CoreWebView2().and_then(|core| {
                    core.CallDevToolsProtocolMethod(*method.as_ref().as_pcwstr(), *params.as_ref().as_pcwstr(), &handler)
                })
            };
            if let Err(e) = call {
                let _ = done.send(Err(e.to_string()));
            }
        })
        .map_err(|e| e.to_string())?;
    result.recv_timeout(Duration::from_secs(10)).map_err(|e| match e {
        RecvTimeoutError::Timeout => "the DevTools call took more than 10 s".to_string(),
        RecvTimeoutError::Disconnected => "the DevTools call sent no result".to_string(),
    })?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_fraction_counts_colored_pixels_only() {
        // Left half orange, right half grey.
        let (w, h) = (64u32, 8u32);
        let mut bgra = Vec::new();
        for _ in 0..h {
            for x in 0..w {
                bgra.extend_from_slice(if x < w / 2 { &[30, 110, 220, 255] } else { &[40, 40, 40, 255] });
            }
        }
        let shot = Shot { width: w, height: h, bgra };
        assert_eq!(shot.scene_fraction([0, 0, 32, 8]), 1.0);
        assert_eq!(shot.scene_fraction([32, 0, 32, 8]), 0.0);
        assert_eq!(shot.scene_fraction([0, 0, 64, 8]), 0.5);
        assert_eq!(shot.bmp().len(), 54 + (w * h * 4) as usize);
    }
}

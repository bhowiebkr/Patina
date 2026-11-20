//! Self-check screenshots: the DevTools protocol of WebView2 renders the page, also off-screen.

use std::sync::mpsc::{channel, RecvTimeoutError};
use std::time::Duration;

use base64::Engine;
use tauri::WebviewWindow;
use webview2_com::{CallDevToolsProtocolMethodCompletedHandler, CoTaskMemPWSTR};
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
    DIB_RGB_COLORS,
};
use windows::Win32::Storage::Xps::{PrintWindow, PRINT_WINDOW_FLAGS, PW_CLIENTONLY};
use windows::Win32::UI::WindowsAndMessaging::{GetClientRect, PW_RENDERFULLCONTENT};

type Png = Result<Vec<u8>, String>;

/// The page color of the GUI guide, in BGR.
const PAGE: [u8; 3] = [0x14; 3];

/// Returns the page as a PNG.
pub fn capture(window: &WebviewWindow) -> Png {
    let (done, result) = channel::<Png>();
    window
        .with_webview(move |webview| {
            let reply = done.clone();
            let handler = CallDevToolsProtocolMethodCompletedHandler::create(Box::new(move |status, json| {
                let _ = reply.send(status.map_err(|e| e.to_string()).and_then(|()| decode(&json)));
                Ok(())
            }));
            let method = CoTaskMemPWSTR::from("Page.captureScreenshot");
            let params = CoTaskMemPWSTR::from(r#"{"format":"png"}"#);
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
        RecvTimeoutError::Timeout => "the screenshot took more than 10 s".to_string(),
        RecvTimeoutError::Disconnected => "the screenshot was not sent".to_string(),
    })?
}

fn decode(json: &str) -> Png {
    let value: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let data = value["data"].as_str().ok_or("the screenshot has no data")?;
    base64::engine::general_purpose::STANDARD.decode(data).map_err(|e| e.to_string())
}

/// Returns the composed client area of the window as a PNG, and the part of `area` (physical x, y,
/// width and height) that the view drew. PrintWindow asks DWM for the composed window, so the
/// image also shows a viewport under the page (15.8).
pub fn print_window(window: &WebviewWindow, area: [f64; 4]) -> Result<(Vec<u8>, (f64, u64)), String> {
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
    // the copy reads them after PrintWindow returns, while the bitmap still exists.
    let mut pixels = unsafe {
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
            Ok((true, bgra)) => bgra,
            Ok((false, _)) => return Err("PrintWindow failed".into()),
            Err(e) => return Err(e.to_string()),
        }
    };
    let (w, h) = (width as usize, height as usize);
    let drawn = drawn(&pixels, w, h, area);
    // The PNG is RGBA. The transparent page leaves an alpha of 0, so set it to opaque.
    for p in pixels.as_chunks_mut::<4>().0 {
        p.swap(0, 2);
        p[3] = 255;
    }
    let mut png = Vec::new();
    let mut encoder = png::Encoder::new(&mut png, w as u32, h as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header().and_then(|mut out| out.write_image_data(&pixels)).map_err(|e| e.to_string())?;
    Ok((png, drawn))
}

/// The part of the BGRA points of `area`, on a grid of 32 x 32, that are not transparent and not
/// the page color. The editor window shows the page color around a viewport window (15.8).
/// The second value is a print of the points: two equal prints show the same image.
fn drawn(pixels: &[u8], width: usize, height: usize, [x, y, w, h]: [f64; 4]) -> (f64, u64) {
    let (mut all, mut drawn, mut print) = (0, 0, 0xcbf2_9ce4_8422_2325_u64);
    for j in 0..32 {
        for i in 0..32 {
            let px = (x + w * f64::from(2 * i + 1) / 64.0) as usize;
            let py = (y + h * f64::from(2 * j + 1) / 64.0) as usize;
            if px < width && py < height {
                let p = &pixels[(py * width + px) * 4..][..4];
                all += 1;
                drawn += usize::from(p[3] != 0 && p[..3] != PAGE);
                // FNV-1a
                print = p[..3].iter().fold(print, |h, &b| (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3));
            }
        }
    }
    (if all == 0 { 0.0 } else { drawn as f64 / all as f64 }, print)
}

#[cfg(test)]
mod tests {
    #[test]
    fn only_opaque_points_that_are_not_the_page_color_are_drawn() {
        // A 4 x 1 image: page, red, transparent red, red. Each pixel gets 8 points of the grid.
        let pixels = [20, 20, 20, 255, 0, 0, 255, 255, 0, 0, 255, 0, 0, 0, 255, 255];
        let drawn = |area| super::drawn(&pixels, 4, 1, area);
        assert_eq!(drawn([0.0, 0.0, 4.0, 1.0]).0, 0.5);
        assert_eq!(drawn([1.0, 0.0, 1.0, 1.0]).0, 1.0);
        assert_eq!(drawn([0.0, 0.0, 1.0, 1.0]).0, 0.0);
        assert_eq!(drawn([8.0, 0.0, 4.0, 1.0]).0, 0.0);
        // The print changes with the image, and the transparent red has the print of the red.
        assert_ne!(drawn([0.0, 0.0, 1.0, 1.0]).1, drawn([1.0, 0.0, 1.0, 1.0]).1);
        assert_eq!(drawn([1.0, 0.0, 1.0, 1.0]).1, drawn([2.0, 0.0, 1.0, 1.0]).1);
    }
}

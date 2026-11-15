//! Bench screenshots: the DevTools protocol of WebView2 renders the page, also off-screen.

use std::sync::mpsc::{channel, RecvTimeoutError};
use std::time::Duration;

use base64::Engine;
use tauri::WebviewWindow;
use webview2_com::{CallDevToolsProtocolMethodCompletedHandler, CoTaskMemPWSTR};

type Png = Result<Vec<u8>, String>;

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

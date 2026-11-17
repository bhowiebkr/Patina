//! rust-analyzer for the Code tab (15.1). The editor starts it on a trusted project and passes the
//! messages of the language server protocol between the page and the process.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use patina_engine::Toolchain;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::engine::lock;
use crate::session::Editor;

/// The rust-analyzer of the open project, or none.
#[derive(Default)]
pub struct Analyzer(Mutex<Option<Running>>);

struct Running {
    child: Child,
    stdin: ChildStdin,
    /// The number of this start. The messages of an older process carry an older number.
    serial: u64,
}

/// A message of rust-analyzer to the page. No text means that the process stopped.
#[derive(Clone, Serialize)]
struct Message {
    serial: u64,
    text: Option<String>,
}

static SERIAL: AtomicU64 = AtomicU64::new(0);

impl Analyzer {
    pub fn stop(&self) {
        if let Some(running) = lock(&self.0).take() {
            end(running);
        }
    }
}

fn end(mut running: Running) {
    let _ = running.child.kill();
    let _ = running.child.wait();
}

/// Starts rust-analyzer of the toolchain in the project root, and returns the number of the start.
/// A project that the user does not trust starts no tool (11).
#[tauri::command(async)]
pub fn analyzer_start(app: AppHandle, editor: State<'_, Editor>) -> Result<u64, String> {
    // The Code tab works without rust-analyzer, so the reason goes only to the Output tab.
    start(app, &editor).inspect_err(|e| eprintln!("rust-analyzer: {e}"))
}

fn start(app: AppHandle, editor: &Editor) -> Result<u64, String> {
    let root = editor.trusted_root("start rust-analyzer")?;
    let toolchain = Toolchain::find(&editor.data.toolchain).map_err(|e| e.to_string())?;
    let program = toolchain.cargo.with_file_name(format!("rust-analyzer{}", std::env::consts::EXE_SUFFIX));
    if !program.is_file() {
        return Err(format!("The toolchain has no rust-analyzer: {} is missing.", program.display()));
    }
    // rust-analyzer finds the crates of the nodes through the workspace manifest.
    patina_engine::write_workspace(&root).map_err(|e| e.to_string())?;
    let mut command = toolchain.command(&program).map_err(|e| e.to_string())?;
    command.env("CARGO", &toolchain.cargo).env("RUSTC", &toolchain.rustc).current_dir(&root);
    // The flags of a build link the build scripts and the procedural macros. The own target folder
    // keeps rust-analyzer and the build from waiting for each other.
    command.env("CARGO_ENCODED_RUSTFLAGS", toolchain.rustflags()).env("CARGO_TARGET_DIR", root.join("build").join("analyzer"));
    command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
    let mut slot = lock(&editor.analyzer.0);
    if let Some(old) = slot.take() {
        end(old);
    }
    let mut child = command.spawn().map_err(|e| format!("rust-analyzer cannot start: {e}"))?;
    let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
        return Err("rust-analyzer has no pipes.".to_owned());
    };
    let serial = SERIAL.fetch_add(1, Ordering::Relaxed) + 1;
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        while let Some(text) = read_message(&mut reader) {
            let _ = app.emit("analyzer", Message { serial, text: Some(text) });
        }
        let _ = app.emit("analyzer", Message { serial, text: None });
    });
    *slot = Some(Running { child, stdin, serial });
    Ok(serial)
}

/// Sends one message to rust-analyzer start `serial`.
#[tauri::command(async)]
pub fn analyzer_send(editor: State<'_, Editor>, serial: u64, text: String) -> Result<(), String> {
    let mut slot = lock(&editor.analyzer.0);
    let running = slot.as_mut().filter(|r| r.serial == serial).ok_or("rust-analyzer does not run.")?;
    write_message(&mut running.stdin, &text).map_err(|e| format!("rust-analyzer: {e}"))
}

#[tauri::command(async)]
pub fn analyzer_stop(editor: State<'_, Editor>) {
    editor.analyzer.stop();
}

/// Writes one message of the base protocol: the `Content-Length` header, a blank line and the text.
fn write_message(to: &mut impl Write, text: &str) -> std::io::Result<()> {
    write!(to, "Content-Length: {}\r\n\r\n{text}", text.len())?;
    to.flush()
}

/// Reads one message of the base protocol. Returns none at the end or at a bad header.
fn read_message(from: &mut impl BufRead) -> Option<String> {
    let mut length = None;
    loop {
        let mut line = String::new();
        if from.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some(value) = line.strip_prefix("Content-Length:") {
            length = value.trim().parse::<usize>().ok();
        }
    }
    let mut body = vec![0; length?];
    from.read_exact(&mut body).ok()?;
    String::from_utf8(body).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_go_through_the_base_protocol() {
        let mut bytes = Vec::new();
        write_message(&mut bytes, "{\"id\":1,\"a\":\"\u{e9}\"}").unwrap();
        write_message(&mut bytes, "{}").unwrap();
        assert!(bytes.starts_with(b"Content-Length: 17\r\n\r\n{"));
        bytes.extend_from_slice(b"Content-Type: x\r\n\r\n");
        let mut from = bytes.as_slice();
        assert_eq!(read_message(&mut from).as_deref(), Some("{\"id\":1,\"a\":\"\u{e9}\"}"));
        assert_eq!(read_message(&mut from).as_deref(), Some("{}"));
        assert_eq!(read_message(&mut from), None);
        assert_eq!(read_message(&mut b"Content-Length: 9\r\n\r\n{}".as_slice()), None);
    }
}

//! Sends the standard output and the standard error of the process to the **Output** tab
//! (15.10). Nodes write there with `println!`, for example the `show` node.

use std::fs::File;
use std::io::{self, Read};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::Duration;

use crate::engine::lock;

/// The size of the read buffer. A longer line goes to the tab in parts.
const BUFFER: usize = 64 * 1024;
/// The most text that waits for the tab. The tab keeps only the newest 5000 lines.
const MAX_WAIT: usize = 256 * 1024;
/// The shortest time between two sends to the tab.
const PERIOD: Duration = Duration::from_millis(100);

/// Sends text to the **Output** tab at most each 100 ms, so a node that prints without a pause
/// does not stop the window (16.1). When too much text waits, the oldest lines are skipped.
#[derive(Clone)]
pub struct Tab(Arc<(Mutex<Waiting>, Condvar)>);

#[derive(Default)]
struct Waiting {
    /// The blocks of text in order, with true for the standard error.
    blocks: Vec<(bool, String)>,
    len: usize,
    skipped: usize,
}

impl Tab {
    /// Starts the thread that calls `send` with each block of text, and true for standard error.
    pub fn start(send: impl Fn(String, bool) + Send + 'static) -> io::Result<Tab> {
        let tab = Tab(Arc::default());
        let shared = tab.0.clone();
        std::thread::Builder::new().name("patina-output".to_owned()).spawn(move || {
            let (waiting, ready) = &*shared;
            loop {
                let next = std::mem::take(&mut *ready.wait_while(lock(waiting), |w| w.blocks.is_empty()).unwrap_or_else(PoisonError::into_inner));
                if next.skipped > 0 {
                    send(format!("Patina skipped {} lines of output, because the nodes printed them too fast.\n", next.skipped), true);
                }
                for (err, text) in next.blocks {
                    send(text, err);
                }
                std::thread::sleep(PERIOD);
            }
        })?;
        Ok(tab)
    }

    /// Adds text for the tab, with true for standard error.
    pub fn push(&self, text: String, err: bool) {
        let (waiting, ready) = &*self.0;
        lock(waiting).push(text, err);
        ready.notify_one();
    }
}

impl Waiting {
    fn push(&mut self, text: String, err: bool) {
        self.len += text.len();
        match self.blocks.last_mut() {
            Some((e, block)) if *e == err => block.push_str(&text),
            _ => self.blocks.push((err, text)),
        }
        if self.len > MAX_WAIT {
            self.cut(self.len - MAX_WAIT / 2);
        }
    }

    /// Removes at least `over` bytes of the oldest text, up to the end of a line.
    fn cut(&mut self, mut over: usize) {
        for (_, block) in &mut self.blocks {
            let end = match block.as_bytes().get(over..).and_then(|rest| rest.iter().position(|&b| b == b'\n')) {
                Some(i) => over + i + 1,
                None => block.len(),
            };
            self.skipped += block[..end].matches('\n').count();
            self.len -= end;
            over = over.saturating_sub(end);
            block.drain(..end);
            if over == 0 {
                break;
            }
        }
        self.blocks.retain(|(_, block)| !block.is_empty());
    }
}

/// Sends the `streams` into pipes, and starts a thread for each pipe: false for the standard
/// output, true for the standard error. `send` gets the text of complete lines, and true for
/// standard error. `send` must not write to the captured streams.
pub fn capture(streams: &[bool], send: impl Fn(String, bool) + Send + Sync + 'static) -> io::Result<()> {
    let send = Arc::new(send);
    for &err in streams {
        let (reader, writer) = io::pipe()?;
        redirect(if err { 2 } else { 1 }, writer)?;
        let send = send.clone();
        std::thread::Builder::new()
            .name(if err { "patina-stderr" } else { "patina-stdout" }.to_owned())
            .spawn(move || read_lines(reader, |text| send(text, err)))?;
    }
    Ok(())
}

/// Takes the standard input and the standard output for the messages of the node process (16.1).
/// The standard input reads the null device after it. The two old handles do not go to child
/// processes, so a child of a node cannot read or write the messages.
pub fn take_stdio() -> io::Result<(File, File)> {
    let input = copy(io::stdin())?;
    let output = copy(io::stdout())?;
    redirect(0, File::open(if cfg!(windows) { "NUL" } else { "/dev/null" })?)?;
    Ok((input, output))
}

/// Sends each block of complete lines that the reader gives. The rest waits for its end of line,
/// so a character in UTF-8 never breaks between two blocks.
pub(crate) fn read_lines(mut reader: impl Read, mut send: impl FnMut(String)) {
    let mut buffer = vec![0; BUFFER];
    let mut len = 0;
    loop {
        match reader.read(&mut buffer[len..]) {
            Ok(0) | Err(_) => break,
            Ok(n) => len += n,
        }
        let end = match buffer[..len].iter().rposition(|&b| b == b'\n') {
            Some(i) => i + 1,
            None if len == buffer.len() => len,
            None => continue,
        };
        send(String::from_utf8_lossy(&buffer[..end]).into_owned());
        buffer.copy_within(end..len, 0);
        len -= end;
    }
    if len > 0 {
        send(String::from_utf8_lossy(&buffer[..len]).into_owned());
    }
}

#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn SetStdHandle(id: u32, handle: *mut std::ffi::c_void) -> i32;
    fn SetHandleInformation(handle: *mut std::ffi::c_void, mask: u32, flags: u32) -> i32;
}

/// Gives a copy of a standard handle that child processes do not get, and stops the old handle
/// from going to child processes.
#[cfg(windows)]
fn copy(stream: impl std::os::windows::io::AsHandle) -> io::Result<File> {
    use std::os::windows::io::AsRawHandle;
    const HANDLE_FLAG_INHERIT: u32 = 1;
    let handle = stream.as_handle();
    // SAFETY: the handle is open. A failure only leaves the flag as it was.
    unsafe { SetHandleInformation(handle.as_raw_handle(), HANDLE_FLAG_INHERIT, 0) };
    Ok(File::from(handle.try_clone_to_owned()?))
}

/// Gives a copy of a standard file descriptor. Child processes do not get the copy (`CLOEXEC`).
#[cfg(unix)]
fn copy(stream: impl std::os::fd::AsFd) -> io::Result<File> {
    Ok(File::from(stream.as_fd().try_clone_to_owned()?))
}

/// Puts `file` in place of standard stream `fd`: 0 for input, 1 for output and 2 for error. Rust
/// code gets the standard handle at each write, also in a cluster library with its own std. C code
/// (`printf`) uses the file descriptor of the C runtime, so it gets the file too.
#[cfg(windows)]
fn redirect(fd: i32, file: impl Into<std::os::windows::io::OwnedHandle>) -> io::Result<()> {
    use std::os::windows::io::IntoRawHandle;
    unsafe extern "C" {
        fn _open_osfhandle(handle: isize, flags: i32) -> i32;
        fn _dup2(old: i32, new: i32) -> i32;
    }
    const O_BINARY: i32 = 0x8000;
    let handle = file.into().into_raw_handle();
    // SAFETY: the handle is open. The process owns it from now on and never closes it.
    if unsafe { SetStdHandle((-10 - fd) as u32, handle) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the handle is open. The new file descriptor owns it and never closes, so the
    // standard handle stays valid. Binary mode keeps each end of line as Rust writes it.
    let crt = unsafe { _open_osfhandle(handle as isize, O_BINARY) };
    // SAFETY: `crt` is open. _dup2 puts a copy of the file in place of `fd`.
    if crt < 0 || unsafe { _dup2(crt, fd) } < 0 {
        return Err(io::Error::other("the C runtime did not take the new standard stream"));
    }
    Ok(())
}

#[cfg(unix)]
fn redirect(fd: i32, file: impl Into<std::os::fd::OwnedFd>) -> io::Result<()> {
    use std::os::fd::IntoRawFd;
    unsafe extern "C" {
        fn dup2(old: i32, new: i32) -> i32;
    }
    let old = file.into().into_raw_fd();
    // SAFETY: both file descriptors are open. dup2 puts a copy of the file in place of `fd`.
    if unsafe { dup2(old, fd) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_complete_lines_go_out() {
        let mut sent = Vec::new();
        let reader = (&b"a\nb"[..]).chain(&b"c\nd\n"[..]);
        read_lines(reader, |text| sent.push(text));
        assert_eq!(sent, ["a\n", "bc\nd\n"]);

        let mut sent = Vec::new();
        read_lines(&b"end"[..], |text| sent.push(text));
        assert_eq!(sent, ["end"], "the text after the last end of line goes out at the end");
    }

    #[test]
    fn too_much_waiting_text_skips_the_oldest_lines() {
        let mut waiting = Waiting::default();
        waiting.push("first\n".to_owned(), true);
        let line = "a line of output\n";
        let count = MAX_WAIT / line.len() + 1;
        for _ in 0..count {
            waiting.push(line.to_owned(), false);
        }
        waiting.push("last\n".to_owned(), true);
        let kept: usize = waiting.blocks.iter().map(|(_, block)| block.len()).sum();
        assert!(waiting.len == kept && kept <= MAX_WAIT && kept >= MAX_WAIT / 2 - line.len(), "{kept}");
        assert_eq!(waiting.skipped + kept / line.len(), count + 1, "each line is kept or counted");
        assert!(matches!(&waiting.blocks[..], [(false, out), (true, last)] if out.starts_with(line) && last == "last\n"));

        let (sent, got) = std::sync::mpsc::channel();
        let tab = Tab::start(move |text, err| {
            let _ = sent.send((text, err));
        })
        .unwrap();
        tab.push("x\n".to_owned(), true);
        assert_eq!(got.recv_timeout(Duration::from_secs(5)).unwrap(), ("x\n".to_owned(), true));
    }

    /// C code reads and writes the redirected streams, not the message pipes (16.1). The test runs
    /// in a child process, because a redirect changes the whole process.
    #[cfg(windows)]
    #[test]
    fn c_code_uses_the_redirected_streams() {
        unsafe extern "C" {
            fn _read(fd: i32, buf: *mut u8, len: u32) -> i32;
            fn _write(fd: i32, buf: *const u8, len: u32) -> i32;
        }
        const NAME: &str = "output::tests::c_code_uses_the_redirected_streams";
        if let Some(path) = std::env::var_os("PATINA_REDIRECT_TEST") {
            redirect(0, File::open("NUL").unwrap()).unwrap();
            redirect(1, File::create(path).unwrap()).unwrap();
            let mut buf = [0; 8];
            // SAFETY: the buffer has 8 bytes.
            let read = unsafe { _read(0, buf.as_mut_ptr(), 8) };
            let text = format!("from C, read {read}\n");
            // SAFETY: the text has `len` bytes.
            unsafe { _write(1, text.as_ptr(), text.len() as u32) };
            return;
        }
        let path = std::env::temp_dir().join(format!("patina-redirect-{}.txt", std::process::id()));
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", NAME, "--test-threads=1"])
            .env("PATINA_REDIRECT_TEST", &path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();
        std::io::Write::write_all(child.stdin.as_mut().unwrap(), b"message\n").unwrap();
        let status = child.wait().unwrap();
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let _ = std::fs::remove_file(&path);
        assert!(status.success() && text.contains("from C, read 0\n"), "{status}: {text}");
    }
}

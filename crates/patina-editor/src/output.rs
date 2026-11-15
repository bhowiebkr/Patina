//! Sends the standard output and the standard error of the process to the **Output** tab
//! (15.10). Nodes write there with `println!`, for example the `show` node.

use std::io::{self, PipeWriter, Read};
use std::sync::Arc;

/// The size of the read buffer. A longer line goes to the tab in parts.
const BUFFER: usize = 64 * 1024;

/// Sends the two streams into pipes, and starts a thread for each pipe. `send` gets the text of
/// complete lines, and true for standard error. `send` must not write to the two streams.
pub fn capture(send: impl Fn(String, bool) + Send + Sync + 'static) -> io::Result<()> {
    let send = Arc::new(send);
    for err in [false, true] {
        let (reader, writer) = io::pipe()?;
        redirect(err, writer)?;
        let send = send.clone();
        std::thread::Builder::new()
            .name(if err { "patina-stderr" } else { "patina-stdout" }.to_owned())
            .spawn(move || read_lines(reader, |text| send(text, err)))?;
    }
    Ok(())
}

/// Sends each block of complete lines that the reader gives. The rest waits for its end of line,
/// so a character in UTF-8 never breaks between two blocks.
fn read_lines(mut reader: impl Read, mut send: impl FnMut(String)) {
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

/// Rust code gets the standard handle at each write, also in a cluster library with its own std.
#[cfg(windows)]
fn redirect(err: bool, writer: PipeWriter) -> io::Result<()> {
    use std::os::windows::io::IntoRawHandle;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn SetStdHandle(id: u32, handle: *mut std::ffi::c_void) -> i32;
    }
    const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;
    const STD_ERROR_HANDLE: u32 = -12i32 as u32;
    let handle = writer.into_raw_handle();
    // SAFETY: the handle is an open pipe. The process owns it from now on and never closes it.
    if unsafe { SetStdHandle(if err { STD_ERROR_HANDLE } else { STD_OUTPUT_HANDLE }, handle) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(unix)]
fn redirect(err: bool, writer: PipeWriter) -> io::Result<()> {
    use std::os::fd::IntoRawFd;
    unsafe extern "C" {
        fn dup2(old: i32, new: i32) -> i32;
    }
    let fd = writer.into_raw_fd();
    // SAFETY: both file descriptors are open. dup2 puts a copy of the pipe in place of 1 or 2.
    if unsafe { dup2(fd, if err { 2 } else { 1 }) } < 0 {
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
}

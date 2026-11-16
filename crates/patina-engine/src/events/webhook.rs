//! The HTTP listener of a `webhook` trigger (spec 9.4). It listens on 127.0.0.1 only.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::time::{Duration, Instant};

/// The longest request line and headers.
const HEAD: usize = 16 * 1024;
/// The longest body.
const BODY: usize = 1024 * 1024;
/// The longest time to read a request, so that a slow client delays the next one by this at most.
const WAIT: Duration = Duration::from_secs(5);
/// The time between two looks for a new connection, so that the listener sees `quit` in time.
const POLL: Duration = Duration::from_millis(20);

/// A status code and its reason.
type Status = (u16, &'static str);

const BAD: Status = (400, "Bad Request");

/// Listens on port `port` of 127.0.0.1.
pub(super) fn bind(port: u16) -> io::Result<TcpListener> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))?;
    listener.set_nonblocking(true)?;
    Ok(listener)
}

/// Answers each request until `quit`. `fire` gets the body of each accepted request, and returns
/// false when the runs ended.
pub(super) fn serve(listener: &TcpListener, path: &str, quit: &AtomicBool, mut fire: impl FnMut(String) -> bool) {
    while !quit.load(Relaxed) {
        match listener.accept() {
            // shortcut: one request at a time, so a slow client delays the next one by WAIT; read in parallel when this matters.
            Ok((stream, _)) => {
                if let Some(body) = answer(stream, path)
                    && !fire(body)
                {
                    return;
                }
            }
            Err(_) => std::thread::sleep(POLL),
        }
    }
}

/// Reads one request from `stream` and answers it. Returns the body of an accepted request.
fn answer(mut stream: TcpStream, path: &str) -> Option<String> {
    let set = stream.set_nonblocking(false).and(stream.set_write_timeout(Some(WAIT)));
    let result = set.map_err(failed).and_then(|()| read(Deadline { stream: &stream, end: Instant::now() + WAIT }, path));
    let (code, reason) = *result.as_ref().err().unwrap_or(&(202, "Accepted"));
    let _ = write!(stream, "HTTP/1.1 {code} {reason}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    result.ok()
}

/// A stream that times out when the whole request takes longer than its end, also when a slow
/// client sends a byte at a time.
struct Deadline<'a> {
    stream: &'a TcpStream,
    end: Instant,
}

impl Read for Deadline<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let left = self.end.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(io::ErrorKind::TimedOut.into());
        }
        self.stream.set_read_timeout(Some(left))?;
        self.stream.read(buf)
    }
}

fn failed(e: io::Error) -> Status {
    if matches!(e.kind(), io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock) { (408, "Request Timeout") } else { BAD }
}

/// Reads a `POST` request to `path` with a body of `Content-Length` bytes. Returns the body as
/// text, with each byte that is not UTF-8 replaced.
fn read(stream: impl Read, path: &str) -> Result<String, Status> {
    let mut reader = BufReader::new(stream);
    let mut head = String::new();
    loop {
        let mut line = Vec::new();
        let limit = (HEAD + 1 - head.len()) as u64;
        let n = (&mut reader).take(limit).read_until(b'\n', &mut line).map_err(failed)?;
        if head.len() + n > HEAD {
            return Err((431, "Request Header Fields Too Large"));
        }
        let line = String::from_utf8(line).map_err(|_| BAD)?;
        if n == 0 || (head.is_empty() && line.trim().is_empty()) {
            return Err(BAD);
        }
        if line.trim().is_empty() {
            break;
        }
        head.push_str(&line);
    }
    let mut lines = head.lines();
    let mut start = lines.next().unwrap_or_default().split_whitespace();
    if start.next() != Some("POST") {
        return Err((405, "Method Not Allowed"));
    }
    if start.next().and_then(|target| target.split('?').next()) != Some(path) {
        return Err((404, "Not Found"));
    }
    let mut length = 0;
    for line in lines {
        let (name, value) = line.split_once(':').ok_or(BAD)?;
        if name.trim().eq_ignore_ascii_case("transfer-encoding") {
            return Err((411, "Length Required"));
        }
        if name.trim().eq_ignore_ascii_case("content-length") {
            length = value.trim().parse::<usize>().map_err(|_| BAD)?;
        }
    }
    if length > BODY {
        return Err((413, "Content Too Large"));
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).map_err(failed)?;
    Ok(String::from_utf8_lossy(&body).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_gives_its_body_or_a_status() {
        let read = |text: &str| read(text.as_bytes(), "/hook");
        assert_eq!(read("POST /hook?a=1 HTTP/1.1\r\nContent-Length: 2\r\n\r\nhi"), Ok("hi".to_owned()));
        assert_eq!(read("POST /hook HTTP/1.1\r\n\r\n"), Ok(String::new()));
        assert_eq!(read("GET /hook HTTP/1.1\r\n\r\n"), Err((405, "Method Not Allowed")));
        assert_eq!(read("POST /other HTTP/1.1\r\n\r\n").unwrap_err().0, 404);
        assert_eq!(read("POST /hook HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n").unwrap_err().0, 411);
        assert_eq!(read("POST /hook HTTP/1.1\r\nContent-Length: 2000000\r\n\r\n").unwrap_err().0, 413);
        assert_eq!(read(&format!("POST /hook HTTP/1.1\r\nX: {}\r\n\r\n", "a".repeat(HEAD))).unwrap_err().0, 431);
        assert_eq!(read("POST /hook HTTP/1.1\r\nContent-Length: 5\r\n\r\nhi").unwrap_err().0, 400);
        assert_eq!(read("POST /hook HTTP/1.1\r\nno colon\r\n\r\n").unwrap_err().0, 400);
    }

    #[test]
    fn a_slow_client_times_out_at_the_end_of_the_request() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (stream, _) = listener.accept().unwrap();
        client.write_all(b"POST / HTTP/1.1
").unwrap();
        let start = Instant::now();
        let slow = Deadline { stream: &stream, end: start + Duration::from_millis(100) };
        assert_eq!(read(slow, "/").unwrap_err().0, 408);
        assert!(start.elapsed() < Duration::from_secs(2));
        let mut done = Deadline { stream: &stream, end: start };
        assert_eq!(done.read(&mut [0; 4]).unwrap_err().kind(), io::ErrorKind::TimedOut);
    }

    #[test]
    fn the_listener_answers_and_fires() {
        let listener = bind(0).unwrap();
        let port = listener.local_addr().unwrap().port();
        let quit = AtomicBool::new(false);
        let mut bodies = Vec::new();
        let send = |request: &str| {
            let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
            stream.write_all(request.as_bytes()).unwrap();
            let mut reply = String::new();
            stream.read_to_string(&mut reply).unwrap();
            reply
        };
        // Sets `quit` also when a check fails, so that the scope ends.
        struct Quit<'a>(&'a AtomicBool);
        impl Drop for Quit<'_> {
            fn drop(&mut self) {
                self.0.store(true, Relaxed);
            }
        }
        let (missing, accepted) = std::thread::scope(|s| {
            let _quit = Quit(&quit);
            s.spawn(|| serve(&listener, "/", &quit, |body| {
                bodies.push(body);
                false
            }));
            (send("POST /x HTTP/1.1\r\n\r\n"), send("POST / HTTP/1.1\r\nContent-Length: 3\r\n\r\nabc"))
        });
        assert!(missing.starts_with("HTTP/1.1 404") && accepted.starts_with("HTTP/1.1 202"), "{missing} {accepted}");
        assert_eq!(bodies, ["abc"]);
    }
}

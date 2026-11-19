//! The node process (16.1). The editor runs the engine in a second process, the same executable
//! with a hidden argument, so a node that crashes does not crash the editor UI. The messages are
//! lines of JSON on the standard input and the standard output of the node process.

pub mod child;
#[cfg(windows)]
pub mod job;

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use patina_engine::{Checked, Parts};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use tauri::{AppHandle, Emitter};

use crate::engine::{Event, Job, lock};
use crate::output;

/// The hidden argument that starts the node process. The toolchain folder comes after it.
pub const ARG: &str = "--node-process";
const STOPPED: &str = "The node process stopped.";
/// A second **Stop** after this time ends the node process when a node still runs. It is a little
/// shorter than the 2 s after which the page shows "Node X does not stop" (15.2).
const STOP_WAIT: Duration = Duration::from_millis(1500);
/// The time that `end` waits for the node process to drop its views.
const DROP_WAIT: Duration = Duration::from_millis(1000);
/// The longest last error text that a crash shows on the run, in bytes.
const TAIL: usize = 1000;

/// The toolchain folder, when this process is the node process.
pub fn arg() -> Option<PathBuf> {
    let mut args = std::env::args_os().skip(1);
    if args.next()? != ARG {
        return None;
    }
    args.next().map(PathBuf::from)
}

/// A message from the editor to the node process.
#[derive(Serialize, Deserialize)]
pub enum ToNode {
    Run(Box<RunParts>),
    Job(Job),
    /// Sets or clears the stop request (15.2).
    Stop(bool),
    /// A question that the node process answers at once, also while a node runs.
    Ask { id: u64, ask: Ask },
}

/// A run of a checked graph. The node process checks the parts again, then runs them.
#[derive(Serialize, Deserialize)]
pub struct RunParts {
    pub serial: u64,
    pub parts: Parts,
    pub build: bool,
    pub slot: Option<String>,
    pub start: bool,
}

#[derive(Serialize, Deserialize)]
pub enum Ask {
    /// The ID of the node that runs (15.2), or an empty ID for project code that names no node
    /// (16.1). The reply is an `Option<String>`.
    Running,
    /// The full text of the live value of an output (15.7). The reply is an `Option<String>`.
    Value(String),
    /// Fires a `trigger` node (9.4). The reply is true when the event domain runs.
    Fire { id: String, text: String },
}

/// A message from the node process. The editor reads the event and the result as raw JSON.
#[derive(Serialize, Deserialize)]
enum FromNode<E, R> {
    Event(E),
    /// A block of text that the nodes wrote to the standard output.
    Output(String),
    Reply { id: u64, result: Result<R, String> },
}

type Reply = Result<Box<RawValue>, String>;

/// Writes the messages of the node process, one line of JSON each. A failed write needs no error,
/// because the editor sees the end of the pipe.
pub struct Out(Mutex<(Box<dyn Write + Send>, Vec<u8>)>);

impl Out {
    pub fn new(writer: impl Write + Send + 'static) -> Out {
        Out(Mutex::new((Box::new(writer), Vec::new())))
    }

    pub fn emit(&self, event: &impl Serialize) {
        if let Err(e) = self.write(&FromNode::<_, ()>::Event(event)) {
            eprintln!("patina: an event of the node process does not go out: {e}");
        }
    }

    pub fn reply<T: Serialize>(&self, id: u64, result: Result<T, String>) {
        if let Err(e) = self.write(&FromNode::<(), _>::Reply { id, result }) {
            let result = Err::<(), _>(format!("The node process cannot send the reply: {e}"));
            let _ = self.write(&FromNode::<(), _>::Reply { id, result });
        }
    }

    pub fn output(&self, text: String) {
        let _ = self.write(&FromNode::<(), ()>::Output(text));
    }

    fn write(&self, message: &impl Serialize) -> serde_json::Result<()> {
        let mut guard = lock(&self.0);
        let (writer, buffer) = &mut *guard;
        buffer.clear();
        serde_json::to_writer(&mut *buffer, message)?;
        buffer.push(b'\n');
        let _ = writer.write_all(buffer).and_then(|()| writer.flush());
        Ok(())
    }
}

/// The editor side of the node process. The first job that needs the process starts it, and so
/// does the first run after a crash.
pub struct Host(Arc<Inner>);

struct Inner {
    app: AppHandle,
    /// The **Output** tab. All output goes through it in order.
    tab: output::Tab,
    toolchain: PathBuf,
    state: Mutex<State>,
    next: AtomicU64,
}

#[derive(Default)]
struct State {
    proc: Option<Proc>,
    /// The time of the first **Stop** since the last run.
    stop_at: Option<Instant>,
    /// The serial of the last run or open, for the event of a crash.
    serial: u64,
}

struct Proc {
    stdin: ChildStdin,
    child: Arc<Mutex<Child>>,
    pending: Pending,
    killed: Arc<AtomicBool>,
    /// When it drops, the processes that the node process started end too.
    #[cfg(windows)]
    _job: Option<job::Job>,
}

/// The reply channels of the open asks. None after the process ended.
type Pending = Arc<Mutex<Option<HashMap<u64, Sender<Reply>>>>>;

impl Host {
    pub fn new(app: AppHandle, toolchain: PathBuf, tab: output::Tab) -> Host {
        Host(Arc::new(Inner { app, tab, toolchain, state: Mutex::default(), next: AtomicU64::new(1) }))
    }

    /// Sends a job when the node process runs. Without the process, no job has work to do.
    pub fn send(&self, job: Job) {
        let _ = self.write(false, false, |_| ToNode::Job(job));
    }

    /// Sends `message` with a new ask ID, and gives the receiver of the reply. None when no node
    /// process runs and `spawn` is false.
    pub fn send_ask(&self, spawn: bool, message: impl FnOnce(u64) -> ToNode) -> Result<Option<Receiver<Reply>>, String> {
        self.write(spawn, true, message)
    }

    /// Sends `message` with a new ask ID, and waits for the reply.
    pub fn ask<T: DeserializeOwned + Default>(&self, spawn: bool, message: impl FnOnce(u64) -> ToNode) -> Result<T, String> {
        wait(self.send_ask(spawn, message)?)
    }

    /// Sends a run of the checked graph, and starts the node process when it does not run. With
    /// `start` false, the clusters load and the graph does not run (15.10).
    pub fn run(&self, serial: u64, checked: Checked, build: bool, slot: Option<String>, start: bool) {
        lock(&self.0.state).serial = serial;
        let parts = checked.into_parts();
        if let Err(e) = self.write(true, false, |_| ToNode::Run(Box::new(RunParts { serial, parts, build, slot, start }))) {
            fail(&self.0, serial, &e, "");
        }
    }

    /// Ends the node process of the last project when project `serial` opens.
    pub fn unload(&self, serial: u64) {
        lock(&self.0.state).serial = serial;
        self.end();
    }

    /// Ends the node process and each process that it started, when a project closes. So no file
    /// of the project stays open, and the user can delete the project folder (16.1).
    pub fn end(&self) {
        // The views drop first, so their surfaces and windows go before the kill (15.8).
        if let Ok(Some(dropped)) = self.send_ask(false, |id| ToNode::Job(Job::DropViews(id))) {
            let _ = dropped.recv_timeout(DROP_WAIT);
        }
        let Some(proc) = lock(&self.0.state).proc.take() else { return };
        proc.killed.store(true, Ordering::SeqCst);
        if let Err(e) = lock(&proc.child).kill() {
            eprintln!("patina: the node process does not end: {e}");
        }
    }

    /// Clears the stop request before a run (15.2).
    pub fn go(&self) {
        lock(&self.0.state).stop_at = None;
        let _ = self.write(false, false, |_| ToNode::Stop(false));
    }

    /// Asks the nodes to stop (15.2). A second **Stop** ends the node process when a node still
    /// runs after `STOP_WAIT`.
    pub fn stop(&self) {
        let first = *lock(&self.0.state).stop_at.get_or_insert_with(Instant::now);
        if first.elapsed() >= STOP_WAIT
            && let Ok(Some(node)) = self.ask::<Option<String>>(false, |id| ToNode::Ask { id, ask: Ask::Running })
        {
            return self.kill(&node);
        }
        let _ = self.write(false, false, |_| ToNode::Stop(true));
    }

    /// Ends the node process, because node `node` does not stop.
    fn kill(&self, node: &str) {
        let (proc, serial) = {
            let mut state = lock(&self.0.state);
            state.stop_at = None;
            (state.proc.take(), state.serial)
        };
        let Some(proc) = proc else { return };
        proc.killed.store(true, Ordering::SeqCst);
        if let Err(e) = lock(&proc.child).kill() {
            eprintln!("patina: the node process does not end: {e}");
        }
        let text = format!("Patina ended the node process, because {} did not stop. The node states are lost.", stuck(node));
        fail(&self.0, serial, &text, "");
    }

    /// Writes a message to the node process. With `spawn`, it starts the process first when it
    /// does not run. With `ask`, the message gets an ID and the receiver of its reply. The reply is
    /// None when no process runs.
    fn write(&self, spawn: bool, ask: bool, message: impl FnOnce(u64) -> ToNode) -> Result<Option<Receiver<Reply>>, String> {
        let mut state = lock(&self.0.state);
        if state.proc.as_ref().is_some_and(|proc| lock(&proc.pending).is_none()) {
            state.proc = None;
            state.stop_at = None;
        }
        if state.proc.is_none() && spawn {
            state.proc = Some(self.spawn().map_err(|e| format!("The node process does not start: {e}"))?);
        }
        let Some(proc) = state.proc.as_mut() else { return Ok(None) };
        let id = self.0.next.fetch_add(1, Ordering::Relaxed);
        let mut line = serde_json::to_vec(&message(id)).map_err(|e| e.to_string())?;
        line.push(b'\n');
        let answer = match ask {
            true => {
                let (reply, answer) = std::sync::mpsc::channel();
                lock(&proc.pending).as_mut().ok_or_else(|| STOPPED.to_owned())?.insert(id, reply);
                Some(answer)
            }
            false => None,
        };
        // A failed write needs no error: the reader sees the end of the process and fails the asks.
        let _ = proc.stdin.write_all(&line).and_then(|()| proc.stdin.flush());
        Ok(answer)
    }

    fn spawn(&self) -> std::io::Result<Proc> {
        let mut command = Command::new(std::env::current_exe()?);
        command.arg(ARG).arg(&self.0.toolchain).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        // CREATE_NO_WINDOW: a debug build has a console, and the node process must not show it.
        #[cfg(windows)]
        std::os::windows::process::CommandExt::creation_flags(&mut command, 0x0800_0000);
        let mut child = command.spawn()?;
        // The node process starts no process before its first job, so a job after the start is in time.
        #[cfg(windows)]
        let limit = job::memory_limit();
        #[cfg(not(windows))]
        let limit = None;
        #[cfg(windows)]
        let job = job::Job::new(&child, limit).inspect_err(|e| eprintln!("patina: the node process has no job object: {e}")).ok();
        #[cfg(windows)]
        let limit = limit.filter(|_| job.is_some());
        let stdin = child.stdin.take().expect("the standard input is piped");
        let stdout = child.stdout.take().expect("the standard output is piped");
        let stderr = child.stderr.take().expect("the standard error is piped");
        let child = Arc::new(Mutex::new(child));
        let pending: Pending = Arc::new(Mutex::new(Some(HashMap::new())));
        let killed = Arc::new(AtomicBool::new(false));
        let tail = Arc::new(Mutex::new(String::new()));
        let errors = {
            let (tab, tail) = (self.0.tab.clone(), tail.clone());
            std::thread::Builder::new().name("patina-node-stderr".to_owned()).spawn(move || {
                output::read_lines(stderr, |text| {
                    keep_tail(&mut lock(&tail), &text);
                    tab.push(text, true);
                })
            })?
        };
        let (inner, read_pending, read_killed, read_child) = (self.0.clone(), pending.clone(), killed.clone(), child.clone());
        std::thread::Builder::new().name("patina-node-stdout".to_owned()).spawn(move || {
            read_messages(&inner, stdout, &read_pending);
            for (_, reply) in lock(&read_pending).take().into_iter().flatten() {
                let _ = reply.send(Err(STOPPED.to_owned()));
            }
            // The last error text of a crash can come a little after the end of the messages.
            let end = Instant::now() + Duration::from_millis(500);
            while !errors.is_finished() && Instant::now() < end {
                std::thread::sleep(Duration::from_millis(10));
            }
            let status = lock(&read_child).wait();
            if !read_killed.load(Ordering::SeqCst) {
                let serial = lock(&inner.state).serial;
                let tail = lock(&tail);
                fail(&inner, serial, &exit_text(status, &tail, limit), &tail);
            }
        })?;
        Ok(Proc {
            stdin,
            child,
            pending,
            killed,
            #[cfg(windows)]
            _job: job,
        })
    }
}

/// Waits for the reply of an ask. No receiver means no node process, so the reply is empty.
pub fn wait<T: DeserializeOwned + Default>(answer: Option<Receiver<Reply>>) -> Result<T, String> {
    let Some(answer) = answer else { return Ok(T::default()) };
    let raw = answer.recv().map_err(|_| STOPPED.to_owned())??;
    serde_json::from_str(raw.get()).map_err(|e| format!("The node process sent a bad reply: {e}"))
}

/// Reads the messages of the node process until its standard output ends.
fn read_messages(inner: &Inner, stdout: ChildStdout, pending: &Pending) {
    let mut reader = BufReader::new(stdout);
    let mut line = Vec::new();
    loop {
        line.clear();
        if !matches!(reader.read_until(b'\n', &mut line), Ok(1..)) {
            return;
        }
        let text = String::from_utf8_lossy(&line);
        match serde_json::from_str::<FromNode<Box<RawValue>, Box<RawValue>>>(&text) {
            Ok(FromNode::Event(event)) => {
                let _ = inner.app.emit("engine", event);
            }
            Ok(FromNode::Output(text)) => inner.tab.push(text, false),
            Ok(FromNode::Reply { id, result }) => {
                let reply = lock(pending).as_mut().and_then(|pending| pending.remove(&id));
                if let Some(reply) = reply {
                    let _ = reply.send(result);
                }
            }
            Err(_) => inner.tab.push(text.into_owned(), false),
        }
    }
}

/// What does not stop: node `node`, or for an empty ID, project code that names no node (16.1).
fn stuck(node: &str) -> String {
    if node.is_empty() { "a node `Default`, a node `Drop` or a view".to_owned() } else { format!("node `{node}`") }
}

/// Shows a failure of the node process in the **Output** tab, and on the run with the last error
/// text `tail`.
fn fail(inner: &Inner, serial: u64, text: &str, tail: &str) {
    inner.tab.push(format!("{text}\n"), true);
    let text = match tail.trim_end() {
        "" => text.to_owned(),
        tail => format!("{text}\n{tail}"),
    };
    let _ = inner.app.emit("engine", Event::Lost { serial, text });
}

/// The text of an end without a request. When the last error text `tail` shows a failed
/// allocation, it also names the memory `limit` of the job (16.1).
fn exit_text(status: std::io::Result<ExitStatus>, tail: &str, limit: Option<usize>) -> String {
    let status = status.map_or_else(|e| e.to_string(), |status| status.to_string());
    let text = format!("The node process stopped without a request ({status}). The node states and the live values are lost. The next run starts a new node process.");
    match limit {
        Some(limit) if tail.contains("memory allocation of") => format!("{text} The memory limit of the node process is {} MB.", limit >> 20),
        _ => text,
    }
}

/// Adds `text` to the last error text, and keeps only its last `TAIL` bytes.
fn keep_tail(tail: &mut String, text: &str) {
    tail.push_str(text);
    if tail.len() > TAIL {
        let mut cut = tail.len() - TAIL;
        while !tail.is_char_boundary(cut) {
            cut += 1;
        }
        tail.drain(..cut);
    }
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use super::*;
    use crate::engine::GuiInput;
    use crate::views::ViewReply;

    #[test]
    fn a_failed_allocation_names_the_memory_limit() {
        let oom = "memory allocation of 268435456 bytes failed\n";
        assert!(exit_text(Err(std::io::ErrorKind::Other.into()), oom, Some(1 << 30)).ends_with(" is 1024 MB."));
        assert!(!exit_text(Err(std::io::ErrorKind::Other.into()), oom, None).contains("MB"));
        assert!(!exit_text(Err(std::io::ErrorKind::Other.into()), "panicked", Some(1 << 30)).contains("MB"));
    }

    #[test]
    fn messages_cross_the_pipe() {
        let input = GuiInput { id: "a".to_owned(), mode: 1, events: "0 1\n".to_owned(), rerun: true };
        let line = serde_json::to_string(&ToNode::Job(Job::Gui(input))).unwrap();
        assert!(matches!(serde_json::from_str(&line).unwrap(), ToNode::Job(Job::Gui(input)) if input.id == "a" && input.mode == 1));

        let (mut reader, writer) = std::io::pipe().unwrap();
        let out = Out::new(writer);
        out.emit(&Event::Started { serial: 3 });
        out.reply(7, Ok(ViewReply { tree: "t".to_owned(), ..ViewReply::default() }));
        out.reply::<()>(8, Err("no".to_owned()));
        out.output("text\n".to_owned());
        drop(out);
        let mut text = String::new();
        reader.read_to_string(&mut text).unwrap();
        let lines: Vec<_> = text.lines().map(|line| serde_json::from_str::<FromNode<Box<RawValue>, Box<RawValue>>>(line).unwrap()).collect();
        assert!(matches!(&lines[0], FromNode::Event(event) if event.get().contains("\"started\"")));
        let FromNode::Reply { id: 7, result } = &lines[1] else { panic!("a reply") };
        let reply: ViewReply = wait(Some({
            let (send, answer) = std::sync::mpsc::channel();
            send.send(result.clone()).unwrap();
            answer
        }))
        .unwrap();
        assert_eq!(reply.tree, "t");
        assert!(matches!(&lines[2], FromNode::Reply { id: 8, result: Err(e) } if e == "no"));
        assert!(matches!(&lines[3], FromNode::Output(text) if text == "text\n"));
        assert_eq!(wait::<bool>(None), Ok(false), "no node process gives the empty reply");
    }

    #[test]
    fn an_empty_id_names_the_code_that_names_no_node() {
        assert_eq!(stuck("a"), "node `a`");
        assert!(stuck("").contains("`Drop`"));
    }

    #[test]
    fn the_tail_keeps_the_last_text() {
        let mut tail = String::new();
        keep_tail(&mut tail, &"\u{e9}".repeat(TAIL));
        keep_tail(&mut tail, "end\n");
        assert!(tail.len() <= TAIL && tail.ends_with("\u{e9}end\n"), "the cut is at a character boundary");
    }
}

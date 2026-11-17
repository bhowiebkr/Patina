//! The node process side (16.1): it reads the messages of the editor and runs the engine thread.

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::sync::Arc;

use super::{Ask, Out, RunParts, ToNode};
use crate::engine::{self, Event, Job, Run, lock};
use crate::output;

/// Runs the node process until the editor closes its standard input.
pub fn run(toolchain: PathBuf) -> ! {
    let (input, output) = output::take_stdio().unwrap_or_else(|e| {
        eprintln!("patina: the node process has no messages: {e}");
        std::process::exit(1);
    });
    let out = Arc::new(Out::new(output));
    let sink = out.clone();
    if let Err(e) = output::capture(&[false], move |text, _| sink.output(text)) {
        eprintln!("patina: the output of the nodes does not go to the editor: {e}");
    }
    let (jobs, shared) = engine::start(out.clone(), toolchain);
    #[cfg(windows)]
    crash::watch(shared.clone());
    let mut input = BufReader::new(input);
    let mut line = String::new();
    loop {
        line.clear();
        if !matches!(input.read_line(&mut line), Ok(1..)) {
            std::process::exit(0);
        }
        let message = match serde_json::from_str::<ToNode>(&line) {
            Ok(message) => message,
            Err(e) => {
                eprintln!("patina: the node process got a bad message: {e}");
                continue;
            }
        };
        match message {
            ToNode::Run(run) => {
                let RunParts { serial, parts, build, slot, start } = *run;
                match parts.check() {
                    Ok(checked) => {
                        let _ = jobs.send(Job::Run(Box::new(Run { serial, checked, build, slot, start })));
                    }
                    Err(e) => out.emit(&Event::Failed { serial, text: e.to_string(), errors: Vec::new() }),
                }
            }
            ToNode::Job(job) => {
                let _ = jobs.send(job);
            }
            ToNode::Stop(stop) => shared.control.set_stop(stop),
            ToNode::Ask { id, ask: Ask::Running } => out.reply(id, Ok(shared.running())),
            ToNode::Ask { id, ask: Ask::Value(from) } => {
                let text = lock(&shared.values).iter().find(|v| v.from == from).map(|v| v.text.clone());
                out.reply(id, Ok(text));
            }
            ToNode::Ask { id, ask: Ask::Fire { id: node, text } } => {
                let remote = lock(&shared.remote).clone();
                out.reply(id, Ok(remote.is_some_and(|remote| remote.fire(&node, text))));
            }
        }
    }
}

/// Writes the node that runs to the standard error when the node process crashes (16.1).
#[cfg(windows)]
mod crash {
    use std::ffi::c_void;
    use std::fs::File;
    use std::io::Write;
    use std::os::windows::io::AsHandle;
    use std::sync::{Arc, OnceLock};

    use crate::engine::Shared;

    type Filter = extern "system" fn(*const c_void) -> i32;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn SetUnhandledExceptionFilter(filter: Option<Filter>) -> Option<Filter>;
    }

    static CRASH: OnceLock<(Arc<Shared>, File)> = OnceLock::new();

    pub fn watch(shared: Arc<Shared>) {
        let Ok(err) = std::io::stderr().as_handle().try_clone_to_owned() else { return };
        let err = File::from(err);
        if CRASH.set((shared, err)).is_ok() {
            // SAFETY: the filter is a plain function that lives for the whole process.
            unsafe { SetUnhandledExceptionFilter(Some(filter)) };
        }
    }

    /// The filter runs on the thread that crashed. It does not allocate and does not wait for a
    /// lock, because the crash can come in the middle of either.
    extern "system" fn filter(_: *const c_void) -> i32 {
        if let Some((shared, file)) = CRASH.get() {
            shared.try_running(|id| {
                let mut file = file;
                let _ = file.write_all(b"patina: node `");
                let _ = file.write_all(id.as_bytes());
                let _ = file.write_all(b"` ran when the node process crashed.\n");
            });
        }
        // EXCEPTION_EXECUTE_HANDLER: the process ends with the code of the crash, without the error report dialog.
        1
    }
}

//! The job object of the node process on Windows (16.1). The processes of the job end when the
//! editor ends, also after a crash of the editor, and when the node process ends. The job also
//! limits the memory of its processes, so a node that leaks memory cannot fill the computer.

use std::ffi::c_void;
use std::io;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::process::Child;

/// The job ends its processes when its last handle closes.
const KILL_ON_JOB_CLOSE: u32 = 0x2000;
/// The job limits the committed memory of all its processes.
const JOB_MEMORY: u32 = 0x200;
/// A process of the job can start a process outside the job with `CREATE_BREAKAWAY_FROM_JOB`.
const BREAKAWAY_OK: u32 = 0x800;
const EXTENDED_LIMIT_INFORMATION: i32 = 9;

/// `JOBOBJECT_EXTENDED_LIMIT_INFORMATION`.
#[repr(C)]
#[derive(Default)]
struct Limits {
    per_process_user_time: i64,
    per_job_user_time: i64,
    flags: u32,
    min_working_set: usize,
    max_working_set: usize,
    active_processes: u32,
    affinity: usize,
    priority_class: u32,
    scheduling_class: u32,
    io: [u64; 6],
    process_memory: usize,
    job_memory: usize,
    peak_process_memory: usize,
    peak_job_memory: usize,
}

/// `MEMORYSTATUSEX`.
#[repr(C)]
#[derive(Default)]
struct MemoryStatus {
    length: u32,
    load: u32,
    total_physical: u64,
    rest: [u64; 6],
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateJobObjectW(attributes: *const c_void, name: *const u16) -> *mut c_void;
    fn SetInformationJobObject(job: *mut c_void, class: i32, info: *const c_void, len: u32) -> i32;
    fn AssignProcessToJobObject(job: *mut c_void, process: *mut c_void) -> i32;
    fn GlobalMemoryStatusEx(status: *mut MemoryStatus) -> i32;
}

/// A job object. Its processes end when it drops.
pub struct Job {
    _handle: OwnedHandle,
}

impl Job {
    /// Puts `child` in a new job. With `memory`, all the processes of the job together can commit
    /// that many bytes at most. A larger allocation fails, so the node process ends.
    pub fn new(child: &Child, memory: Option<usize>) -> io::Result<Job> {
        // SAFETY: null attributes and a null name make an unnamed job that child processes do not get.
        let raw = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if raw.is_null() {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: the handle is open, and the job owns it from now on.
        let job = Job { _handle: unsafe { OwnedHandle::from_raw_handle(raw) } };
        let flags = KILL_ON_JOB_CLOSE | BREAKAWAY_OK | if memory.is_some() { JOB_MEMORY } else { 0 };
        let limits = Limits { flags, job_memory: memory.unwrap_or(0), ..Limits::default() };
        // SAFETY: `limits` has the layout of the information class and lives during the call.
        let set = unsafe { SetInformationJobObject(raw, EXTENDED_LIMIT_INFORMATION, (&raw const limits).cast(), size_of::<Limits>() as u32) };
        // SAFETY: the job and the process handles are open.
        if set == 0 || unsafe { AssignProcessToJobObject(raw, child.as_raw_handle()) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(job)
    }
}

/// The memory limit of the node process: three quarters of the physical memory. None when
/// Windows does not tell the size.
pub fn memory_limit() -> Option<usize> {
    let mut status = MemoryStatus { length: size_of::<MemoryStatus>() as u32, ..MemoryStatus::default() };
    // SAFETY: `status` has the layout of MEMORYSTATUSEX and the length that it needs.
    if unsafe { GlobalMemoryStatusEx(&raw mut status) } == 0 || status.total_physical == 0 {
        return None;
    }
    Some(usize::try_from(status.total_physical / 4 * 3).unwrap_or(usize::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, Write};
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    /// The test runs child processes of the test executable, because a job changes the process.
    #[test]
    fn the_job_limits_memory_and_ends_its_processes() {
        const NAME: &str = "node_process::job::tests::the_job_limits_memory_and_ends_its_processes";
        if let Ok(what) = std::env::var("PATINA_JOB_TEST") {
            // The child waits until it is in the job.
            std::io::stdin().lock().read_line(&mut String::new()).unwrap();
            match what.as_str() {
                "alloc" => assert_eq!(vec![1u8; 256 << 20][(256 << 20) - 1], 1),
                _ => std::thread::sleep(Duration::from_secs(60)),
            }
            return;
        }
        assert!(memory_limit().is_some_and(|m| m > 0));
        let start = |what: &str, memory| {
            let mut child = Command::new(std::env::current_exe().unwrap())
                .args(["--exact", NAME, "--test-threads=1"])
                .env("PATINA_JOB_TEST", what)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap();
            let job = Job::new(&child, memory).unwrap();
            child.stdin.take().unwrap().write_all(b"go\n").unwrap();
            (child, job)
        };

        let (mut child, _job) = start("alloc", Some(64 << 20));
        assert!(!child.wait().unwrap().success(), "the job stops an allocation over its limit");

        let (mut child, job) = start("sleep", None);
        drop(job);
        let end = Instant::now() + Duration::from_secs(10);
        while child.try_wait().unwrap().is_none() {
            assert!(Instant::now() < end, "the process ends with its job");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

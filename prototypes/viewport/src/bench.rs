//! The bench: each run loads the page with one frame path, and the page runs the scenarios and
//! the checks. Rust adds the render counters, the CPU and the GPU use, and writes the report.
// shortcut: copied from the connections prototype; merge into one crate when a third prototype needs it.

use std::path::PathBuf;
use std::process::Child;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use crate::stats::round2;

/// The frame paths: a native surface under the web view, or a copy into the page.
pub const MODES: [&str; 3] = ["native", "shared", "channel"];
const WATCHDOG: Duration = Duration::from_secs(60);

pub struct Bench {
    pub runs: Vec<usize>,
    pub measure_s: u32,
    pub shots: bool,
    pub focus_events: AtomicU32,
    heartbeat: Mutex<Instant>,
    state: Mutex<State>,
}

struct State {
    index: usize,
    results: Vec<Value>,
    scenarios: Vec<Value>,
    sampler: Option<Child>,
}

impl Bench {
    pub fn new(runs: Vec<usize>, measure_s: u32, shots: bool) -> Self {
        Self {
            runs,
            measure_s,
            shots,
            focus_events: AtomicU32::new(0),
            heartbeat: Mutex::new(Instant::now()),
            state: Mutex::new(State { index: 0, results: Vec::new(), scenarios: Vec::new(), sampler: None }),
        }
    }

    pub fn current(&self) -> Option<&'static str> {
        let index = self.state.lock().unwrap().index;
        self.runs.get(index).map(|&r| MODES[r])
    }

    pub fn query(&self, mode: &str) -> String {
        format!("bench=1&mode={mode}&measure={}&shots={}", self.measure_s, u8::from(self.shots))
    }

    pub fn beat(&self) {
        *self.heartbeat.lock().unwrap() = Instant::now();
    }

    /// Stops the process when the page stops sending bench calls.
    pub fn watch(self: &Arc<Self>) {
        let bench = Arc::clone(self);
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_secs(1));
            if bench.heartbeat.lock().unwrap().elapsed() > WATCHDOG {
                eprintln!("bench: the page sent no bench call for {} s", WATCHDOG.as_secs());
                std::process::exit(1);
            }
        });
    }

    pub fn scenario_begin(&self) {
        self.beat();
        let mut state = self.state.lock().unwrap();
        if let Some(mut old) = state.sampler.take() {
            let _ = old.kill();
        }
        state.sampler = spawn_sampler(self.measure_s.saturating_sub(1).max(1));
    }

    /// Merges the page counters, the render counters and the CPU and GPU use of a scenario.
    pub fn scenario_end(&self, name: &str, page: Value, render: Value) {
        self.beat();
        let sampler = self.state.lock().unwrap().sampler.take();
        let usage = sampler.and_then(collect_sampler).unwrap_or(Value::Null);
        let mut merged = json!({ "name": name });
        for part in [render, page] {
            if let Value::Object(map) = part {
                merged.as_object_mut().unwrap().extend(map);
            }
        }
        if let (Some(cpu), Some(wall)) = (usage["cpu_seconds"].as_f64(), usage["wall_seconds"].as_f64()) {
            merged["cpu_cores"] = json!(round2(cpu / wall.max(0.001)));
        }
        if let Some(gpu) = usage["gpu"].as_array() {
            let values: Vec<f64> = gpu.iter().filter_map(Value::as_f64).collect();
            merged["gpu_3d_percent"] = json!(round2(values.iter().sum::<f64>() / values.len().max(1) as f64));
        }
        self.state.lock().unwrap().scenarios.push(merged);
    }

    /// Ends the current run. `info` holds the page facts and the checks. Returns true when
    /// another run follows.
    pub fn run_done(&self, info: Value) -> bool {
        self.beat();
        let mut state = self.state.lock().unwrap();
        let mode = MODES[self.runs[state.index]];
        let scenarios = std::mem::take(&mut state.scenarios);
        state.results.push(json!({ "mode": mode, "info": info, "scenarios": scenarios }));
        state.index += 1;
        state.index < self.runs.len()
    }

    /// Writes the report under `artifacts/viewport/` and prints a table.
    pub fn finish(&self) -> std::io::Result<PathBuf> {
        let state = self.state.lock().unwrap();
        let unix = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let report = json!({
            "unix_time": unix,
            "measure_seconds": self.measure_s,
            "focus_events": self.focus_events.load(Ordering::SeqCst),
            "cpu_threads": std::thread::available_parallelism().map_or(0, |n| n.get()),
            "runs": state.results,
        });
        print_table(&state.results);
        write_artifact(&format!("bench-{unix}.json"), serde_json::to_string_pretty(&report)?.as_bytes())
    }
}

/// Writes a file to `artifacts/viewport/` and returns its path.
pub fn write_artifact(name: &str, data: &[u8]) -> std::io::Result<PathBuf> {
    // shortcut: the path comes from the source tree, so the bench runs only from a checkout.
    let dir = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../artifacts/viewport"));
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(name);
    std::fs::write(&path, data)?;
    Ok(path)
}

fn print_table(runs: &[Value]) {
    let cols = [
        "size", "view_fps", "page_fps", "interval_ms_p99", "work_ms_p95", "native_latency_ms_p50",
        "page_latency_ms_p50", "page_latency_ms_p95", "copy_mb_per_s", "dropped", "cpu_cores", "gpu_3d_percent",
    ];
    println!("{:<8} {:<8} {}", "mode", "scenario", cols.join(" | "));
    for run in runs {
        let mode = run["mode"].as_str().unwrap_or("?");
        for s in run["scenarios"].as_array().into_iter().flatten() {
            let cells: Vec<String> = cols.iter().map(|c| s[*c].to_string()).collect();
            println!("{mode:<8} {:<8} {}", s["name"].as_str().unwrap_or("?"), cells.join(" | "));
        }
        if let Some(checks) = run["info"]["checks"].as_object() {
            let failed: Vec<&String> = checks.iter().filter(|(_, v)| v["ok"] != json!(true)).map(|(k, _)| k).collect();
            println!("{mode:<8} checks   {} of {} pass; failed: {failed:?}", checks.len() - failed.len(), checks.len());
        }
    }
}

/// Reads the CPU time of this process tree and the 3D engine use of its GPU counters.
#[cfg(windows)]
const SAMPLER: &str = r"
$ErrorActionPreference = 'SilentlyContinue'
$all = Get-CimInstance Win32_Process
$ids = [System.Collections.Generic.List[int]]::new(); $ids.Add($root)
for ($i = 0; $i -lt $ids.Count; $i++) {
  foreach ($p in $all) { if ($p.ParentProcessId -eq $ids[$i] -and $p.ProcessId -ne $PID) { $ids.Add([int]$p.ProcessId) } }
}
function Cpu { $s = 0.0; foreach ($id in $ids) { $p = Get-Process -Id $id; if ($p) { $s += $p.TotalProcessorTime.TotalSeconds } }; $s }
$keys = @($ids | ForEach-Object { 'pid_' + $_ + '_' })
$c0 = Cpu; $t0 = [DateTime]::UtcNow
$gpu = @(Get-Counter '\GPU Engine(*engtype_3D)\Utilization Percentage' -SampleInterval 1 -MaxSamples $secs | ForEach-Object {
  $s = 0.0; foreach ($x in $_.CounterSamples) { foreach ($k in $keys) { if ($x.InstanceName.StartsWith($k)) { $s += $x.CookedValue } } }; [math]::Round($s, 2) })
$c1 = Cpu; $t1 = [DateTime]::UtcNow
[ordered]@{ pids = @($ids); cpu_seconds = $c1 - $c0; wall_seconds = ($t1 - $t0).TotalSeconds; gpu = $gpu } | ConvertTo-Json -Compress
";

#[cfg(windows)]
fn spawn_sampler(secs: u32) -> Option<Child> {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let script = format!("$root = {}; $secs = {}; {}", std::process::id(), secs, SAMPLER);
    Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()
}

// shortcut: CPU and GPU use are measured on Windows only; add /proc sampling for Linux when needed.
#[cfg(not(windows))]
fn spawn_sampler(_secs: u32) -> Option<Child> {
    None
}

fn collect_sampler(child: Child) -> Option<Value> {
    let out = child.wait_with_output().ok()?;
    serde_json::from_slice(&out.stdout).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scenario_end_merges_the_page_over_the_render_counters() {
        let bench = Bench::new(vec![0], 1, false);
        bench.scenario_end("one", json!({ "a": 2, "page_fps": 60 }), json!({ "a": 1, "view_fps": 59 }));
        let state = bench.state.lock().unwrap();
        assert_eq!(state.scenarios[0], json!({ "name": "one", "a": 2, "page_fps": 60, "view_fps": 59 }));
    }
}

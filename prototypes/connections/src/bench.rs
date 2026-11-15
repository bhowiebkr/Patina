//! The bench: each run loads the page with one renderer and one feed, and the page runs the
//! scenarios. Rust adds the send times, the CPU and the GPU use, and writes the report.

use std::path::PathBuf;
use std::process::Child;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

pub struct Run {
    pub renderer: &'static str,
    pub feed: &'static str,
    /// The device pixel ratio to emulate with the page zoom. 0 keeps the display scale.
    pub dpr: f64,
    pub scenarios: &'static str,
}

const ALL: &str = "idle-far,idle-near,pan,zoom,select,drag";
pub const RUNS: [Run; 8] = [
    Run { renderer: "svg", feed: "raw", dpr: 0.0, scenarios: ALL },
    Run { renderer: "webgl", feed: "raw", dpr: 0.0, scenarios: ALL },
    Run { renderer: "svg", feed: "off", dpr: 0.0, scenarios: "idle-near,pan" },
    Run { renderer: "webgl", feed: "off", dpr: 0.0, scenarios: "idle-near,pan" },
    Run { renderer: "webgl", feed: "json", dpr: 0.0, scenarios: "idle-near" },
    Run { renderer: "svg", feed: "raw", dpr: 1.0, scenarios: "idle-near,pan" },
    Run { renderer: "svg", feed: "raw", dpr: 2.0, scenarios: "idle-near,pan" },
    Run { renderer: "webgl", feed: "raw", dpr: 2.0, scenarios: "idle-near,pan" },
];

const WATCHDOG: Duration = Duration::from_secs(90);

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
            state: Mutex::new(State {
                index: 0,
                results: Vec::new(),
                scenarios: Vec::new(),
                sampler: None,
            }),
        }
    }

    pub fn current(&self) -> Option<&'static Run> {
        let index = self.state.lock().unwrap().index;
        self.runs.get(index).map(|&r| &RUNS[r])
    }

    pub fn query(&self, run: &Run) -> String {
        format!(
            "bench=1&renderer={}&feed={}&scenarios={}&measure={}&shots={}",
            run.renderer,
            run.feed,
            run.scenarios,
            self.measure_s,
            u8::from(self.shots)
        )
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
        state.sampler = spawn_sampler(self.measure_s.saturating_sub(2).max(1));
    }

    pub fn scenario_end(&self, mut stats: Value, send_us: Vec<u32>, frame_bytes: usize) {
        self.beat();
        let sampler = self.state.lock().unwrap().sampler.take();
        let usage = sampler.and_then(collect_sampler).unwrap_or(Value::Null);
        let mut sorted = send_us;
        sorted.sort_unstable();
        stats["rust_send_us_p50"] = json!(percentile(&sorted, 0.5));
        stats["rust_send_us_p99"] = json!(percentile(&sorted, 0.99));
        stats["rust_frames_sent"] = json!(sorted.len());
        stats["frame_bytes"] = json!(frame_bytes);
        if let (Some(cpu), Some(wall)) =
            (usage["cpu_seconds"].as_f64(), usage["wall_seconds"].as_f64())
        {
            stats["cpu_cores"] = json!(round2(cpu / wall.max(0.001)));
        }
        if let Some(gpu) = usage["gpu"].as_array() {
            let values: Vec<f64> = gpu.iter().filter_map(Value::as_f64).collect();
            let mean = values.iter().sum::<f64>() / values.len().max(1) as f64;
            stats["gpu_3d_percent"] = json!(round2(mean));
        }
        stats["processes"] = usage["pids"].as_array().map_or(json!(0), |p| json!(p.len()));
        self.state.lock().unwrap().scenarios.push(stats);
    }

    /// Ends the current run. `info` holds the page facts, for example the GPU and the DPR.
    /// Returns true when another run follows.
    pub fn run_done(&self, info: Value) -> bool {
        self.beat();
        let mut state = self.state.lock().unwrap();
        let run = &RUNS[self.runs[state.index]];
        let scenarios = std::mem::take(&mut state.scenarios);
        state.results.push(json!({
            "renderer": run.renderer,
            "feed": run.feed,
            "info": info,
            "scenarios": scenarios,
        }));
        state.index += 1;
        state.index < self.runs.len()
    }

    /// Writes the report under `artifacts/connections/` and prints a table.
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

/// Writes a file to `artifacts/connections/` and returns its path.
pub fn write_artifact(name: &str, data: &[u8]) -> std::io::Result<PathBuf> {
    // shortcut: the path comes from the source tree, so the bench runs only from a checkout.
    let dir = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../artifacts/connections"));
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(name);
    std::fs::write(&path, data)?;
    Ok(path)
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn percentile(sorted: &[u32], p: f64) -> u32 {
    if sorted.is_empty() {
        return 0;
    }
    sorted[((sorted.len() - 1) as f64 * p).round() as usize]
}

fn print_table(runs: &[Value]) {
    let cols = [
        "fps", "interval_p50", "interval_p99", "dropped", "work_p95", "latency_p50",
        "cpu_cores", "gpu_3d_percent",
    ];
    println!("{:<20} {:<10} {}", "run", "scenario", cols.join(" | "));
    for run in runs {
        let dpr = &run["info"]["dpr"];
        let label = format!("{}/{}@{dpr}", run["renderer"], run["feed"]).replace('"', "");
        for s in run["scenarios"].as_array().into_iter().flatten() {
            let cells: Vec<String> = cols.iter().map(|c| s[*c].to_string()).collect();
            println!("{label:<20} {:<10} {}", s["name"].as_str().unwrap_or("?"), cells.join(" | "));
        }
    }
}

/// Reads the CPU time of this process tree and the 3D engine use of its GPU counters.
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
    fn percentile_picks_the_nearest_rank() {
        assert_eq!(percentile(&[], 0.5), 0);
        assert_eq!(percentile(&[1, 2, 3, 4, 5], 0.5), 3);
        assert_eq!(percentile(&[1, 2, 3, 4, 5], 0.99), 5);
    }
}

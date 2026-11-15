//! The counters of the render thread. The vectors have a fixed capacity, so the frame loop does
//! not allocate. The bench reads a summary and resets the counters for each scenario.

use std::time::Instant;

use serde_json::{json, Value};

/// Room for 60 s of samples at 60 FPS with two viewports.
const CAPACITY: usize = 8192;

pub struct Samples(Vec<f64>);

impl Samples {
    fn new() -> Self {
        Self(Vec::with_capacity(CAPACITY))
    }

    /// Adds a sample. A full buffer ignores it.
    pub fn push(&mut self, value: f64) {
        if self.0.len() < self.0.capacity() {
            self.0.push(value);
        }
    }

    fn percentile(&self, p: f64) -> f64 {
        let mut sorted = self.0.clone();
        sorted.sort_by(f64::total_cmp);
        percentile(&sorted, p)
    }
}

/// The counters of one frame. The render thread merges them under the lock one time a frame.
#[derive(Default)]
pub struct Frame {
    pub upload_bytes: u64,
    pub copies: u64,
    pub copy_bytes: u64,
    pub dropped: u64,
    pub view_frames: u64,
    pub presents: u64,
}

pub struct Stats {
    pub adapter: String,
    pub surface_format: String,
    pub since: Instant,
    pub frames: u64,
    pub sum: Frame,
    pub work_ms: Samples,
    pub interval_ms: Samples,
    pub latency_ms: Samples,
    pub readback_ms: Samples,
    pub keys: u64,
    pub surfaces_configured: u64,
    pub device_lost: u32,
    pub recovering_since: Option<Instant>,
    pub recovery_ms: Samples,
    last_frame: Option<Instant>,
}

impl Stats {
    pub fn new() -> Self {
        Self {
            adapter: String::new(),
            surface_format: String::new(),
            since: Instant::now(),
            frames: 0,
            sum: Frame::default(),
            work_ms: Samples::new(),
            interval_ms: Samples::new(),
            latency_ms: Samples::new(),
            readback_ms: Samples::new(),
            keys: 0,
            surfaces_configured: 0,
            device_lost: 0,
            recovering_since: None,
            recovery_ms: Samples::new(),
            last_frame: None,
        }
    }

    pub fn frame_done(&mut self, begin: Instant, frame: &Frame) {
        self.frames += 1;
        self.work_ms.push(begin.elapsed().as_secs_f64() * 1000.0);
        if let Some(last) = self.last_frame {
            self.interval_ms.push(begin.duration_since(last).as_secs_f64() * 1000.0);
        }
        self.last_frame = Some(begin);
        let s = &mut self.sum;
        s.upload_bytes += frame.upload_bytes;
        s.copies += frame.copies;
        s.copy_bytes += frame.copy_bytes;
        s.dropped += frame.dropped;
        s.view_frames += frame.view_frames;
        s.presents += frame.presents;
    }

    /// Clears the counters of a scenario. The device facts and the loss count stay.
    pub fn reset(&mut self) {
        let keep = (std::mem::take(&mut self.adapter), std::mem::take(&mut self.surface_format));
        let (lost, recovery) = (self.device_lost, std::mem::replace(&mut self.recovery_ms, Samples::new()));
        *self = Self::new();
        (self.adapter, self.surface_format) = keep;
        self.device_lost = lost;
        self.recovery_ms = recovery;
    }

    pub fn summary(&self) -> Value {
        let secs = self.since.elapsed().as_secs_f64().max(0.001);
        let s = &self.sum;
        let per_copy = |v: u64| v.checked_div(s.copies).unwrap_or(0);
        json!({
            "adapter": self.adapter,
            "surface_format": self.surface_format,
            "seconds": round2(secs),
            "loop_fps": round2(self.frames as f64 / secs),
            "view_fps": round2(s.view_frames as f64 / secs),
            "presents": s.presents,
            "work_ms_p50": round2(self.work_ms.percentile(0.5)),
            "work_ms_p95": round2(self.work_ms.percentile(0.95)),
            "interval_ms_p50": round2(self.interval_ms.percentile(0.5)),
            "interval_ms_p99": round2(self.interval_ms.percentile(0.99)),
            "native_latency_ms_p50": round2(self.latency_ms.percentile(0.5)),
            "native_latency_ms_p95": round2(self.latency_ms.percentile(0.95)),
            "native_latency_samples": self.latency_ms.0.len(),
            "readback_wait_ms_p50": round2(self.readback_ms.percentile(0.5)),
            "readback_wait_ms_p95": round2(self.readback_ms.percentile(0.95)),
            "copies": s.copies,
            "copy_bytes_per_frame": per_copy(s.copy_bytes),
            "copy_mb_per_s": round2(s.copy_bytes as f64 / secs / 1e6),
            "dropped": s.dropped,
            "upload_bytes_per_frame": s.upload_bytes.checked_div(self.frames).unwrap_or(0),
            "keys": self.keys,
            "surfaces_configured": self.surfaces_configured,
            "device_lost": self.device_lost,
            "recovery_ms": self.recovery_ms.0.iter().map(|v| round2(*v)).collect::<Vec<_>>(),
        })
    }
}

pub fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

/// The nearest-rank percentile of sorted values. Empty input gives 0.
pub fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    sorted[((sorted.len() - 1) as f64 * p).round() as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_stop_at_the_capacity_and_give_percentiles() {
        let mut s = Samples::new();
        for i in 0..CAPACITY + 10 {
            s.push(i as f64);
        }
        assert_eq!(s.0.len(), CAPACITY);
        assert_eq!(s.percentile(0.0), 0.0);
        assert_eq!(s.percentile(1.0), (CAPACITY - 1) as f64);
        assert_eq!(percentile(&[], 0.5), 0.0);
    }

    #[test]
    fn reset_keeps_the_device_facts() {
        let mut stats = Stats::new();
        stats.adapter = "gpu".into();
        stats.device_lost = 1;
        stats.frames = 9;
        stats.reset();
        assert_eq!((stats.adapter.as_str(), stats.device_lost, stats.frames), ("gpu", 1, 0));
    }
}

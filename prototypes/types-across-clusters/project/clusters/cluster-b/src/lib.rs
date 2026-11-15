//! Cluster B of the prototype project: the `sink` node.
//!
//! Patina generates the glue of each cluster (spec 8.2). This file shows what the glue holds.

use types::abi::{self, INPUT, InputArg, READ, SinkSummary, TAKE};
use types::geometry::{Point, Probe};
use types::units2::Meters;
use types::{pin, read, take};

types::cluster_exports![
    pin::<String>("greeting", INPUT, READ),
    pin::<Vec<Point>>("points", INPUT, TAKE),
    pin::<Probe>("probe", INPUT, TAKE),
    pin::<Meters>("distance", INPUT, TAKE),
];

/// Runs the node on four inputs, in pin order, and writes what it found to `out`.
/// An input with a null pointer has no connection. Returns 0, or -1 for bad arguments.
///
/// # Safety
///
/// Each input must hold a value of the type of its pin. The node takes the values of its
/// `TAKE` pins, so the engine must forget them.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn patina_sink_run(
    args: *const InputArg,
    len: usize,
    out: *mut SinkSummary,
) -> i32 {
    if args.is_null() || out.is_null() || len != 4 {
        return -1;
    }
    // SAFETY: the caller promises four inputs of the pin types.
    let args = unsafe { std::slice::from_raw_parts(args, len) };
    let mut summary = SinkSummary::default();
    if let Some(text) = unsafe { read::<String>(&args[0]) } {
        summary.seen |= abi::SEEN_GREETING;
        summary.greeting_len = sink::greeting_len(text) as u64;
        summary.greeting_ok = u8::from(text == "hello from cluster A");
    }
    if let Some(points) = unsafe { take::<Vec<Point>>(&args[1]) } {
        summary.seen |= abi::SEEN_POINTS;
        let (len, x, y) = sink::sum_points(points);
        summary.points_len = len as u64;
        summary.sum_x = x;
        summary.sum_y = y;
    }
    if let Some(probe) = unsafe { take::<Probe>(&args[2]) } {
        summary.seen |= abi::SEEN_PROBE;
        summary.probe_label_len = sink::take_probe(probe) as u64;
        summary.live_after_probe = types::geometry::live_probes();
    }
    if let Some(distance) = unsafe { take::<Meters>(&args[3]) } {
        summary.seen |= abi::SEEN_DISTANCE;
        summary.distance = sink::distance_m(distance);
    }
    // SAFETY: the caller promises a valid `out`.
    unsafe { out.write(summary) };
    0
}

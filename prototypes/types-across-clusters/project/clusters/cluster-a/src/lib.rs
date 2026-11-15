//! Cluster A of the prototype project: the `source` node.
//!
//! Patina generates the glue of each cluster (spec 8.2). This file shows what the glue holds.

use types::abi::{ErasedValue, OUTPUT, TAKE};
use types::geometry::{Point, Probe};
use types::units1::Meters;
use types::{erase, pin};

types::cluster_exports![
    pin::<String>("greeting", OUTPUT, TAKE),
    pin::<Vec<Point>>("points", OUTPUT, TAKE),
    pin::<Probe>("probe", OUTPUT, TAKE),
    pin::<Meters>("distance", OUTPUT, TAKE),
];

/// Runs the node and moves its four outputs, in pin order, into `out`.
/// Returns the number of outputs, or 0 if `out` has no room for them.
///
/// # Safety
///
/// `out` must be null or point to room for `cap` values.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn patina_source_run(out: *mut ErasedValue, cap: usize) -> usize {
    if out.is_null() || cap < 4 {
        return 0;
    }
    let values = [
        erase(source::greeting()),
        erase(source::points()),
        erase(source::probe()),
        erase(source::distance()),
    ];
    for (i, value) in values.into_iter().enumerate() {
        // SAFETY: `i < 4 <= cap`.
        unsafe { out.add(i).write(value) };
    }
    4
}

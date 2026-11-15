//! A local type crate (spec 5.1).

use std::sync::atomic::{AtomicU64, Ordering};

/// A point. Build 2 of the prototype changes the two fields to `f32`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: u32,
    pub y: u32,
}

impl Point {
    pub fn from_ints(x: u16, y: u16) -> Self {
        Point {
            x: x.into(),
            y: y.into(),
        }
    }
}

static LIVE: AtomicU64 = AtomicU64::new(0);
static DROPPED: AtomicU64 = AtomicU64::new(0);

/// A type with `Drop`. One copy of this crate keeps one count for all clusters.
pub struct Probe {
    pub label: String,
}

impl Probe {
    pub fn new(label: &str) -> Self {
        LIVE.fetch_add(1, Ordering::SeqCst);
        Probe {
            label: label.to_owned(),
        }
    }
}

impl Drop for Probe {
    fn drop(&mut self) {
        LIVE.fetch_sub(1, Ordering::SeqCst);
        DROPPED.fetch_add(1, Ordering::SeqCst);
    }
}

pub fn live_probes() -> u64 {
    LIVE.load(Ordering::SeqCst)
}

pub fn dropped_probes() -> u64 {
    DROPPED.load(Ordering::SeqCst)
}

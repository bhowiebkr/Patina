//! The C ABI between the engine and a cluster (spec 5.2).
//!
//! The engine and the shared types library of the project each compile this file. Only
//! `#[repr(C)]` types cross the boundary, so the two sides can use different Rust versions.

use std::ffi::c_void;

/// Change this number when a type in this file changes.
pub const ABI_VERSION: u32 = 1;

pub const INPUT: u8 = 0;
pub const OUTPUT: u8 = 1;
/// The node reads the value (`&T`, spec 4.4).
pub const READ: u8 = 0;
/// The node takes the value (`T`, spec 4.4).
pub const TAKE: u8 = 1;

/// A type key (spec 5.1).
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct TypeKey {
    pub lo: u64,
    pub hi: u64,
}

impl std::fmt::Display for TypeKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:016x}{:016x}", self.hi, self.lo)
    }
}

/// A value that the engine holds but never reads (spec 5.2).
#[repr(C)]
pub struct ErasedValue {
    pub ptr: *mut c_void,
    pub key: TypeKey,
    /// The drop function of the library that made the value.
    pub drop: unsafe extern "C" fn(*mut c_void),
}

/// One input of a run. A null `ptr` means that the input has no connection.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct InputArg {
    pub ptr: *mut c_void,
    pub key: TypeKey,
}

impl InputArg {
    /// An input without a connection.
    pub const NONE: InputArg = InputArg {
        ptr: std::ptr::null_mut(),
        key: TypeKey { lo: 0, hi: 0 },
    };
}

pub const NAME_LEN: usize = 32;
pub const TYPE_NAME_LEN: usize = 96;

/// One pin of a cluster, as the cluster sees it.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PinDesc {
    pub name: [u8; NAME_LEN],
    pub type_name: [u8; TYPE_NAME_LEN],
    pub dir: u8,
    pub mode: u8,
    pub size: u64,
    pub align: u64,
    pub key: TypeKey,
}

impl PinDesc {
    pub const EMPTY: PinDesc = PinDesc {
        name: [0; NAME_LEN],
        type_name: [0; TYPE_NAME_LEN],
        dir: 0,
        mode: 0,
        size: 0,
        align: 0,
        key: TypeKey { lo: 0, hi: 0 },
    };

    pub fn new(
        name: &str,
        dir: u8,
        mode: u8,
        type_name: &str,
        size: usize,
        align: usize,
        key: TypeKey,
    ) -> Self {
        PinDesc {
            name: fixed(name),
            type_name: fixed(type_name),
            dir,
            mode,
            size: size as u64,
            align: align as u64,
            key,
        }
    }

    pub fn name(&self) -> &str {
        unfixed(&self.name)
    }

    pub fn type_name(&self) -> &str {
        unfixed(&self.type_name)
    }
}

fn fixed<const N: usize>(text: &str) -> [u8; N] {
    let mut out = [0; N];
    let len = text.len().min(N - 1);
    out[..len].copy_from_slice(&text.as_bytes()[..len]);
    out
}

fn unfixed(bytes: &[u8]) -> &str {
    let len = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    std::str::from_utf8(&bytes[..len]).unwrap_or("?")
}

/// The counters of the `Probe` type in one copy of the `geometry` crate.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct ProbeStats {
    pub live: u64,
    pub dropped: u64,
}

pub const SEEN_GREETING: u8 = 1;
pub const SEEN_POINTS: u8 = 2;
pub const SEEN_PROBE: u8 = 4;
pub const SEEN_DISTANCE: u8 = 8;

/// What the sink node of cluster B found in its inputs.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct SinkSummary {
    pub seen: u8,
    pub greeting_ok: u8,
    pub greeting_len: u64,
    pub points_len: u64,
    pub sum_x: f64,
    pub sum_y: f64,
    pub probe_label_len: u64,
    pub live_after_probe: u64,
    pub distance: f64,
}

/// A 128-bit FNV-1a hash for type keys and build IDs.
///
/// Each part has a length prefix, so the parts ("ab", "c") and ("a", "bc") give other keys.
pub struct KeyHasher(u128);

impl KeyHasher {
    const OFFSET: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
    const PRIME: u128 = 0x0000_0000_0100_0000_0000_0000_0000_013b;

    pub fn new() -> Self {
        KeyHasher(Self::OFFSET)
    }

    fn bytes(&mut self, data: &[u8]) {
        for byte in data {
            self.0 ^= u128::from(*byte);
            self.0 = self.0.wrapping_mul(Self::PRIME);
        }
    }

    pub fn part(&mut self, data: &[u8]) -> &mut Self {
        self.bytes(&(data.len() as u64).to_le_bytes());
        self.bytes(data);
        self
    }

    pub fn key(&self) -> TypeKey {
        TypeKey {
            lo: self.0 as u64,
            hi: (self.0 >> 64) as u64,
        }
    }
}

impl Default for KeyHasher {
    fn default() -> Self {
        Self::new()
    }
}

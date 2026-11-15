//! The shared types library of the prototype project (spec 5.2).
//!
//! Patina generates this crate. Before each build, the harness writes `Cargo.toml`, `abi.rs` and
//! `keys.rs` next to this file. The clusters link to this library and to `std` as dynamic
//! libraries (`-C prefer-dynamic`), so they use one copy of each type.

use std::any::type_name;
use std::ffi::c_void;
use std::mem::{align_of, size_of};

pub mod abi;
mod keys;

pub use geometry;
pub use units1;
pub use units2;

use abi::{ErasedValue, InputArg, KeyHasher, PinDesc, ProbeStats, TypeKey};

/// What the type key needs to know about the crate of a type (spec 5.1).
pub struct CrateInfo {
    pub name: &'static str,
    pub version: &'static str,
    pub build_id: &'static str,
}

/// A type that a pin can carry. Patina implements it for the public types of the type crates.
pub trait PinType: 'static {
    fn type_key() -> TypeKey;
}

/// Returns the type key of `T` (spec 5.1). `params` are the keys of the type parameters.
pub fn key_of<T: 'static>(krate: &CrateInfo, params: &[TypeKey]) -> TypeKey {
    let mut hasher = KeyHasher::new();
    hasher
        .part(krate.name.as_bytes())
        .part(krate.version.as_bytes())
        .part(type_name::<T>().as_bytes())
        .part(&(size_of::<T>() as u64).to_le_bytes())
        .part(&(align_of::<T>() as u64).to_le_bytes())
        .part(keys::TOOLCHAIN.as_bytes())
        .part(krate.build_id.as_bytes());
    for param in params {
        hasher.part(&param.lo.to_le_bytes()).part(&param.hi.to_le_bytes());
    }
    hasher.key()
}

impl PinType for String {
    fn type_key() -> TypeKey {
        key_of::<Self>(&keys::STD, &[])
    }
}

impl<T: PinType> PinType for Vec<T> {
    fn type_key() -> TypeKey {
        key_of::<Self>(&keys::STD, &[T::type_key()])
    }
}

impl PinType for geometry::Point {
    fn type_key() -> TypeKey {
        key_of::<Self>(&keys::GEOMETRY, &[])
    }
}

impl PinType for geometry::Probe {
    fn type_key() -> TypeKey {
        key_of::<Self>(&keys::GEOMETRY, &[])
    }
}

impl PinType for units1::Meters {
    fn type_key() -> TypeKey {
        key_of::<Self>(&keys::UNITS_1, &[])
    }
}

impl PinType for units2::Meters {
    fn type_key() -> TypeKey {
        key_of::<Self>(&keys::UNITS_2, &[])
    }
}

/// Describes a pin of type `T`.
pub fn pin<T: PinType>(name: &str, dir: u8, mode: u8) -> PinDesc {
    PinDesc::new(
        name,
        dir,
        mode,
        type_name::<T>(),
        size_of::<T>(),
        align_of::<T>(),
        T::type_key(),
    )
}

/// Moves a value into an erased value. The drop function is code of the calling library.
pub fn erase<T: PinType>(value: T) -> ErasedValue {
    ErasedValue {
        ptr: Box::into_raw(Box::new(value)).cast(),
        key: T::type_key(),
        drop: drop_boxed::<T>,
    }
}

unsafe extern "C" fn drop_boxed<T>(ptr: *mut c_void) {
    // SAFETY: `erase::<T>` made `ptr` with `Box::into_raw`, and the engine drops it once.
    drop(unsafe { Box::from_raw(ptr.cast::<T>()) });
}

/// Reads the value of an input. Returns `None` if the input has no connection.
///
/// # Safety
///
/// The input must hold a `T` that lives for `'a`.
pub unsafe fn read<'a, T>(arg: &InputArg) -> Option<&'a T> {
    // SAFETY: the caller promises a live `T`.
    (!arg.ptr.is_null()).then(|| unsafe { &*arg.ptr.cast::<T>() })
}

/// Takes the value of an input. Returns `None` if the input has no connection.
///
/// # Safety
///
/// The input must hold an erased `T` that the engine gave away.
pub unsafe fn take<T>(arg: &InputArg) -> Option<T> {
    // SAFETY: the caller promises an erased `T` from `erase`, and the engine forgets it.
    (!arg.ptr.is_null()).then(|| *unsafe { Box::from_raw(arg.ptr.cast::<T>()) })
}

/// Writes the pins to the buffer of the engine, and returns the number of pins.
///
/// # Safety
///
/// `out` must be null or point to room for `cap` pins.
pub unsafe fn write_pins(pins: &[PinDesc], out: *mut PinDesc, cap: usize) -> usize {
    if !out.is_null() {
        for (i, pin) in pins.iter().take(cap).enumerate() {
            // SAFETY: `i < cap`.
            unsafe { out.add(i).write(*pin) };
        }
    }
    pins.len()
}

/// Returns the counters of the one copy of `geometry` in this library.
pub fn probe_stats() -> ProbeStats {
    ProbeStats {
        live: geometry::live_probes(),
        dropped: geometry::dropped_probes(),
    }
}

/// Makes the exports that each cluster gives to the engine.
#[macro_export]
macro_rules! cluster_exports {
    ($($pin:expr),* $(,)?) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn patina_abi_version() -> u32 {
            $crate::abi::ABI_VERSION
        }

        /// # Safety
        ///
        /// `out` must be null or point to room for `cap` pins.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn patina_pins(out: *mut $crate::abi::PinDesc, cap: usize) -> usize {
            let pins = [$($pin),*];
            // SAFETY: the caller promises room for `cap` pins.
            unsafe { $crate::write_pins(&pins, out, cap) }
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn patina_probe_stats() -> $crate::abi::ProbeStats {
            $crate::probe_stats()
        }

        /// A hash of the Rust `TypeId` of `Point`, made in the code of this cluster.
        #[unsafe(no_mangle)]
        pub extern "C" fn patina_point_type_id() -> u64 {
            use ::std::hash::{Hash, Hasher};
            let mut hasher = ::std::hash::DefaultHasher::new();
            ::std::any::TypeId::of::<$crate::geometry::Point>().hash(&mut hasher);
            hasher.finish()
        }
    };
}

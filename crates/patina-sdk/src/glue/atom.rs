//! A number parameter, or a number value that goes from one task to another (9.2). It has no lock.
//! The scheduler starts a task only after the tasks that it reads from, so a read never races a
//! write of the same frame. A race with a new parameter value gives the old or the new value.

use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};

/// A value that fits in 64 bits and a flag: a scalar, or an `Option` of a scalar.
pub trait Bits: Copy {
    fn to_bits(self) -> (u64, bool);
    fn from_bits(bits: u64, some: bool) -> Self;
}

macro_rules! bits {
    ($($t:ty: $to:expr, $from:expr;)*) => {$(
        impl Bits for $t {
            #[inline(always)]
            fn to_bits(self) -> (u64, bool) {
                ($to(self), true)
            }
            #[inline(always)]
            fn from_bits(bits: u64, _: bool) -> $t {
                $from(bits)
            }
        }
        impl Bits for Option<$t> {
            #[inline(always)]
            fn to_bits(self) -> (u64, bool) {
                self.map_or((0, false), |v| ($to(v), true))
            }
            #[inline(always)]
            fn from_bits(bits: u64, some: bool) -> Option<$t> {
                some.then(|| $from(bits))
            }
        }
    )*};
}

// The casts keep all bits: a value goes back to its own type with the same bits.
bits! {
    bool: u64::from, |b| b != 0;
    char: |c| u64::from(u32::from(c)), |b| char::from_u32(b as u32).unwrap_or_default();
    f32: |f: f32| u64::from(f.to_bits()), |b| f32::from_bits(b as u32);
    f64: f64::to_bits, f64::from_bits;
    i8: |v| v as u64, |b| b as i8;
    i16: |v| v as u64, |b| b as i16;
    i32: |v| v as u64, |b| b as i32;
    i64: |v| v as u64, |b| b as i64;
    isize: |v| v as u64, |b| b as isize;
    u8: u64::from, |b| b as u8;
    u16: u64::from, |b| b as u16;
    u32: u64::from, |b| b as u32;
    u64: |v| v, |b| b;
    usize: |v| v as u64, |b| b as usize;
}

/// The cell. `read` and `write` give guards, as `Shared` does, so the generated code is the same.
pub struct Atom<T> {
    bits: AtomicU64,
    some: AtomicBool,
    kind: PhantomData<T>,
}

impl<T: Bits> Atom<T> {
    pub fn new(value: T) -> Atom<T> {
        let (bits, some) = value.to_bits();
        Atom { bits: AtomicU64::new(bits), some: AtomicBool::new(some), kind: PhantomData }
    }

    #[inline(always)]
    pub fn get(&self) -> T {
        T::from_bits(self.bits.load(Relaxed), self.some.load(Relaxed))
    }

    #[inline(always)]
    pub fn set(&self, value: T) {
        let (bits, some) = value.to_bits();
        self.bits.store(bits, Relaxed);
        self.some.store(some, Relaxed);
    }

    #[inline(always)]
    pub fn read(&self) -> AtomRead<T> {
        AtomRead(self.get())
    }

    /// The guard stores its value when it drops.
    #[inline(always)]
    pub fn write(&self) -> AtomWrite<'_, T> {
        AtomWrite { atom: self, value: self.get() }
    }
}

/// A copy of the value of an `Atom`.
pub struct AtomRead<T>(T);

impl<T> Deref for AtomRead<T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &T {
        &self.0
    }
}

/// A copy of the value of an `Atom`, which goes back into the `Atom` when the guard drops.
pub struct AtomWrite<'a, T: Bits> {
    atom: &'a Atom<T>,
    value: T,
}

impl<T: Bits> Deref for AtomWrite<'_, T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &T {
        &self.value
    }
}

impl<T: Bits> DerefMut for AtomWrite<'_, T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut T {
        &mut self.value
    }
}

impl<T: Bits> Drop for AtomWrite<'_, T> {
    #[inline(always)]
    fn drop(&mut self) {
        self.atom.set(self.value);
    }
}

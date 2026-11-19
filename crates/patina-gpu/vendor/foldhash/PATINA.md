# Patina patch

Source: foldhash 0.2.0 from crates.io. The original Zlib license applies.

RandomState keeps the original SharedSeed pointer.
This keeps hash results stable when a map crosses loaded Rust libraries.
The upstream state stores no pointer and reads the calling library's global seed.
That breaks lookups in a shared wgpu device after another cluster loads.

RandomState grows from 8 to 16 bytes. The hashing algorithm is unchanged.
All Patina project libraries must use this patch.

The Cargo manifest omits benchmark dependencies and benchmark targets.
The entropy function casts its function pointer through a raw pointer for Rust 1.100.
The README table borders use ASCII.
Original source SHA256:

60338e1ca7bfc781724819cd82ef4a9bd61f4f5c17772b3c0f776fb2257c60dc  src/convenience.rs
807f098a619303f9e4d5cf582e0eb3b25c14451a36fa4e7a4ab440d3c95bf29e  src/fast.rs
777319e7f2919bede856e13594169cfc4bdbaf2e541463da58c86e7e832dc45f  src/lib.rs
33073e1869642a0c8689cfe3e8a1ecdc0a5c939b4e0a9ba03c111babf5b92849  src/quality.rs
d4abbbbd7ca41f1dec07c0da3463b767ebdd953c0140eb94db617a7e3151cbe3  src/seed.rs

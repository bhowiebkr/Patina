# Prototype: geometry kernel

This is task D0 of the [workbench plan](../../docs/workbenches-plan.md). It finds a geometry
kernel for the model workbench. The measure cases are a gun of 30 parts and a city block of 500
buildings. The result is also in [STATUS.md](../../STATUS.md).

## What it has

The crate is the baseline with no kernel and no dependency:

- `Seed` and `Rng`: `seed.child("barrel")` gives a sub-seed for a named part. The steps use
  integers only.
- `Affine` and `Mesh`: closed prisms, boxes and cylinders, transforms, merges with a part ID for
  each triangle, a closed check, the volume and a hash.
- `sample::gun` and `sample::block`: the two measure cases.

The baseline has no booleans. Merged parts can overlap, and it cannot cut a bore.

## Run it

```bash
cargo run -p geometry --release
```

## Results

All runs were on Windows 11, in release, with Rust 1.100.0 and the MSVC target. A game ran in
the background, so the times are high. The kernel trials were throwaway crates outside the
repository. Nobody ran them in a type crate of a workbench or with the Patina toolchain.

| Candidate | Gun of 30 parts | Block of 500 | 2500 buildings | Build | License |
| --- | --- | --- | --- | --- | --- |
| Baseline (this crate) | 9.1 us, no cut | 0.43 ms | 1.7 ms | under 1 s | MIT OR Apache-2.0 |
| Manifold 3.5.5 (`manifold-csg` 0.4.2) | 2.7 ms, with the bore cut | 81 ms | 557 ms | 21 s C++, then 3.0 s | Apache-2.0 |
| monstertruck 0.4.2 | fails, 5 of 5 seeds | about 1.7 s | not run | 14.6 s | Apache-2.0 |
| csgrs 0.20.1 | not built | not built | not built | does not resolve | MIT |
| cadrum (OCCT) | not built | not built | not built | not built | MIT, OCCT LGPL-2.1 |

- Baseline: each part is closed, and the 2500 buildings pass the closed check (12 ms). The hash
  of gun 42 is `6fca71f3709c227b`.
- Manifold: all results are valid manifolds. The gun has 655 triangles, and the block of 500 has
  22000 triangles with the mesh output. The runs used one thread.
  - The default build of `manifold-csg-sys` does a full `git clone` of Manifold (78 MB), oneTBB
    (60 MB) and Clipper2 (12 MB). Then it runs CMake. The full clones were too slow here.
  - The trial used shallow clones, a CMake build by hand (2.8 s configure, 17.8 s compile)
    without TBB, and `MANIFOLD_CSG_LIB_DIR`. The Rust build was 3.0 s, the rebuild 0.4 s, and the
    executable 549 KB.
  - Clipper2 has the BSL-1.0 license. oneTBB has Apache-2.0, and only the `parallel` feature
    needs it.
- monstertruck: pure Rust B-rep, with no CMake. The rebuild was 2.7 s, and the executable 2.2 MB.
  - The gun union failed at part 8 to 12, with `NotClosedShell` or `EmptyOutputShell`.
  - A building of 3 boxes took 3.3 ms and was closed.
- csgrs: Cargo cannot resolve it, because its dependency `core2` 0.4 is yanked.
- cadrum: its build downloads a prebuilt OCCT of 39 MB from a small repository of one person. The
  trial did not run this binary. The source build of OCCT needs CMake. OCCT gives fillets and
  STEP.

## Recommendation

This is an agent decision of 2026-11-19. The owner can change it.

- Use Manifold for the booleans of the model workbench. It was the only candidate that cut the
  gun, and it is fast enough for the city target of task E8 (2 s).
- Keep the kernel behind the type crate of the workbench. The tool gets no kernel code.
- The type crate must not need CMake or git (spec 13.4). Next, build the Manifold and Clipper2
  sources with the `cc` crate and the C++ compiler of the toolchain. Spec 13.2 makes Zig that
  compiler for `x86_64-pc-windows-gnu`.
- Keep the pure Rust mesh of this crate for the primitives, the previews and the tests.
- Do not use monstertruck now. Look at it again for fillets when it is more stable.

## Not checked

- The build and the link with the Patina toolchain: the gnu target, Zig and `rust-lld`. This
  computer has only the MSVC target, and no Zig outside a toolchain run.
- A build in a type crate of a workbench, and a build in a node cluster.
- Linux and macOS. The hash can change on another platform, because `sin` and `cos` come from
  the platform math library.
- The `parallel` feature of Manifold with oneTBB.

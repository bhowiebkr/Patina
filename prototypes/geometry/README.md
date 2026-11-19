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
repository. Nobody ran them in a type crate of a workbench. The run with the Patina toolchain is
in the D0b section.

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
- The type crate must not need CMake or git (spec 13.4). It builds the Manifold and Clipper2
  sources with the `cc` crate and Zig as the C++ compiler. The D0b section has the result.
- Keep the pure Rust mesh of this crate for the primitives, the previews and the tests.
- Do not use monstertruck now. Look at it again for fillets when it is more stable.

## D0b: Manifold with the Patina toolchain

The second trial built Manifold 3.5.5 with no CMake and no git, on 2026-11-19. It was also a
throwaway crate outside the repository.

- The `build.rs` compiles 19 Manifold sources, the 5 sources of its C bindings and 4 Clipper2
  sources with the `cc` crate. It defines `MANIFOLD_PAR=-1` and `MANIFOLD_CROSS_SECTION`.
- The sources need about 1.1 MB for Manifold (`src`, `include`, `bindings/c`) and 325 KB for
  Clipper2 (`include`, `src`).
- MSVC target: the cold build took 5 s, and a gun took 3.6 ms.
- Patina toolchain of a toolchain run (Rust 1.100.0, `x86_64-pc-windows-gnu`, Zig 0.17.1 as the
  C++ compiler, `rust-lld`, a clean environment):
  - The cold build took 8 s with an empty Zig cache, and a rebuild 1 s.
  - A gun of 30 parts with the bore cut took 3.1 ms, with 754 triangles. All 200 guns were valid.
  - The executable is 1.9 MB. It needs only system DLLs, with `msvcrt.dll` as the C runtime.

The toolchain has no C++ setup now. The trial added these parts by hand:

- `CXX` for the target: a Zig wrapper that runs `zig c++ -target x86_64-windows-gnu`. Without
  it, the `cc` crate looks for `g++`.
- `cpp_link_stdlib(None)`, and `libc++.a` and `libc++abi.a` from the Zig cache. Zig builds them
  when it links a C++ program. The `cc` crate links `stdc++` by default, and the toolchain has no
  libstdc++ for Zig.
- `-Cdlltool` with the Zig dlltool, as the engine does. `getrandom` needs it, through the
  `parallel` feature of `cc`.
- A shim of 12 lines. libc++ of Zig imports 8 functions from the UCRT: `mbrtowc`, `mbrlen`,
  `mbsrtowcs`, `wcrtomb`, `wctob`, `btowc`, `_strftime_l` and `__sys_nerr`. The Rust gnu target
  links msvcrt, so `rust-lld` cannot find these imports. The shim points each import at the
  msvcrt function. `_strftime_l` ignores the locale.

## Not checked

- C++ exceptions across the link: libc++abi of Zig with the unwinder of the Rust gnu target.
  The trial threw no exception.
- A build in a type crate of a workbench, and a build in a node cluster.
- Linux and macOS. The hash can change on another platform, because `sin` and `cos` come from
  the platform math library.
- The `parallel` feature of Manifold with oneTBB.
